use crate::peer::Peer;
use nu_json::Map;
use std::fs;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process;
use tempfile::Builder;

mod cfg_file_modify;
mod clap_args;
mod defaults;
mod latency;
mod parse_config;
mod parsing_peers;
mod peer;
mod resolve;
mod unpack;
mod using_api;
mod version;

fn main() {
    let matches = clap_args::build_args();

    let print_only = matches.get_flag("print");
    let update_cfg = matches.get_flag("update_cfg");
    let use_api = matches.get_flag("api");

    if !(print_only || update_cfg || use_api) {
        println!("Parameters expected: '-p' or '-u' and (or) '-a'.");
        println!("For more information try '-h'.");
        println!("Nothing to do, exit.");
        process::exit(0);
    }

    let conf_path = match matches.get_one::<PathBuf>("config") {
        Some(_c) => _c,
        _ => {
            eprintln!("Can't get the configuration file default path.");
            process::exit(1);
        }
    };

    if !print_only {
        // Checking if the file exists
        if !conf_path.exists() {
            eprintln!("The Yggdrasil configuration file does not exist.");
            process::exit(1);
        }

        // Checking write access to the configuration file
        let _t = match check_permissions(&conf_path) {
            Ok(_ro) => _ro,
            Err(e) => {
                eprintln!(
                    "There is no write access to the Yggdrasil configuration file ({}).",
                    e
                );
                process::exit(1);
            }
        };
    }

    // Creating a temporary directory
    let tmp_dir = match create_tmp_dir() {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Failed to create a temporary directory ({}).", e);
            process::exit(1);
        }
    };

    // Download the archive with peers
    let _res = match download_archive(&tmp_dir) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Failed to download archive with peers ({}).", e);
            process::exit(1);
        }
    };

    // Unpacking the downloaded archive
    let _res = match crate::unpack::unpack_archive(&tmp_dir) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Failed to unpack archive ({}).", e);
            process::exit(1);
        }
    };

    // Deleting unnecessary files
    let _ret = fs::remove_file(std::path::Path::new(
        format!("{}/public-peers-master/README.md", &tmp_dir.display()).as_str(),
    ));
    let _ret = fs::remove_file(std::path::Path::new(
        format!("{}/peers.zip", &tmp_dir.display()).as_str(),
    ));
    let _ret = fs::remove_dir_all(std::path::Path::new(
        format!("{}/public-peers-master/other", &tmp_dir.display()).as_str(),
    ));

    let peers_dir: PathBuf =
        std::path::Path::new(format!("{}/public-peers-master/", &tmp_dir.display()).as_str())
            .to_path_buf();

    // Collecting peers in a vector
    let mut peers: Vec<Peer> = Vec::new();
    match crate::parsing_peers::collect_peers(&peers_dir, &mut peers) {
        Ok(_r) => _r,
        Err(e) => {
            eprintln!("Couldn't get peer addresses from downloaded files ({}).", e);
            process::exit(1);
        }
    };

    // Deleting unnecessary files
    let _ret = fs::remove_dir_all(std::path::Path::new(tmp_dir.as_path()));

    // Calculating latency
    std::thread::scope(|scope| {
        for peer in &mut peers {
            scope.spawn(move || {
                crate::latency::set_latency(peer);
            });
        }
    });

    //Sorting the vector
    peers.sort_by(|a, b| a.latency.cmp(&b.latency));

    // Printing data
    if print_only {
        println!(
            "{0:<60}|{1:<15}|{2:<15}|{3:<10}",
            "URI", "Region", "Country", "Latency"
        );
        println!("{0:-<100}", "-");
        for peer in peers {
            if !peer.is_alive {
                break;
            }
            println!(
                "{0:<60}|{1:<15}|{2:<15}|{3:<10}",
                peer.uri, peer.region, peer.country, peer.latency
            );
        }
        process::exit(0);
    } else if update_cfg || use_api {
        if let Some(number) = matches.get_one::<String>("number") {
            let n_peers: u8 = match number.parse() {
                Ok(_n) => _n,
                Err(e) => {
                    eprintln!(
                        "The number of peers must be in the range from 0 to 255 ({}).",
                        e
                    );
                    process::exit(1);
                }
            };

            //Reading the configuration file
            let cfg_txt = match parse_config::read_config(conf_path) {
                Ok(_ct) => _ct,
                Err(e) => {
                    eprintln!("The configuration file cannot be read ({}).", e);
                    process::exit(1);
                }
            };

            let exrta_peers: Option<&String> = matches.get_one::<String>("extra");
            let ignored_peers: Option<&String> = matches.get_one::<String>("ignore");

            // Adding peers to the configuration file
            if update_cfg {
                cfg_file_modify::add_peers_to_conf_new(
                    &peers,
                    conf_path,
                    n_peers,
                    exrta_peers,
                    ignored_peers,
                    &cfg_txt,
                );
            }

            //Restart if required
            if matches.get_flag("restart") {
                #[cfg(not(target_os = "windows"))]
                let _ = std::process::Command::new("systemctl")
                    .arg("restart")
                    .arg("yggdrasil")
                    .spawn();

                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("net")
                        .arg("stop")
                        .arg("yggdrasil")
                        .output();
                    let _ = std::process::Command::new("net")
                        .arg("start")
                        .arg("yggdrasil")
                        .spawn();
                }
            }

            // Adding peers during execution
            if use_api {
                //Parsing the configuration file
                let mut conf_obj: Map<String, nu_json::Value> =
                    match parse_config::get_hjson_obj(&cfg_txt) {
                        Ok(co) => co,
                        Err(e) => {
                            eprintln!("Can't parse the config file ({})!", e);
                            process::exit(1);
                        }
                    };

                using_api::update_peers(&peers, &mut conf_obj, n_peers, exrta_peers, ignored_peers);
            }
        }
    }
}

fn check_permissions(path: &PathBuf) -> io::Result<bool> {
    let md = fs::metadata(path)?;
    let permissions = md.permissions();
    Ok(permissions.readonly())
}

fn create_tmp_dir() -> io::Result<PathBuf> {
    let tmp_dir = Builder::new().prefix("peers_updater_").tempdir()?;
    Ok(tmp_dir.into_path())
}

fn download_archive(tmp_dir: &PathBuf) -> io::Result<bool> {
    let mut resp = reqwest::blocking::get(
        "https://github.com/yggdrasil-network/public-peers/archive/refs/heads/master.zip",
    )
    .expect("request failed");
    let mut out = File::create(format!("{}/peers.zip", tmp_dir.display()))?;
    io::copy(&mut resp, &mut out)?;
    Ok(true)
}
