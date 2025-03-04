use crate::peer::Peer;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub fn add_peers_to_conf_new(
    peers: &Vec<Peer>,

    conf_path: &PathBuf,
    n_peers: u8,
    always_in_p: Option<&String>,
    ignored_peers: Option<&String>,
    cfg_txt: &str,
) {
    let mut char_vec: Vec<char> = cfg_txt.chars().collect();
    let vec_len = char_vec.len();

    let peers_start_pos = find_peers_start_pos(&char_vec, 1, vec_len);
    let peers_end_pos = find_end_of_peers_fragment(&char_vec, peers_start_pos + 6, vec_len);

    if !(peers_start_pos < peers_end_pos) {
        eprintln!("Incorrect configuration file format. The file was not written to.");
        return;
    }

    let mut new_peers = String::from("Peers:\n  [");

    let mut n_added: u8 = 0;
    for peer in peers {
        if let Some(ignored_peers_p) = ignored_peers {
            if ignored_peers_p.contains(&peer.uri) {
                continue;
            }
        }
        new_peers.push_str(
            format!("\n    #{}/{}\n    {}", peer.region, peer.country, peer.uri).as_str(),
        );
        n_added += 1;
        if n_added == n_peers {
            break;
        }
    }

    //Always in
    if let Some(always_in) = always_in_p {
        let ai = always_in.split(" ");
        new_peers.push_str("\n\n    #extra");
        for ai_s in ai {
            new_peers.push_str(format!("\n    {}", ai_s).as_str());
        }
    }

    new_peers.push_str("\n  ]");

    char_vec.splice(peers_start_pos..peers_end_pos + 1, new_peers.chars());

    if let Ok(mut f) = File::create(&conf_path) {
        let _ = match f.write_all(char_vec.into_iter().collect::<String>().as_bytes()) {
            Ok(_) => {}
            Err(e) => {
                eprintln!(
                    "The changes could not be written to the configuration file ({}).",
                    e
                );
            }
        };
    } else {
        eprintln!("The changes could not be written to the configuration file.");
    }
}

fn find_peers_start_pos(chars: &Vec<char>, from: usize, to: usize) -> usize {
    let mut cur_pos = from;

    while cur_pos <= to {
        if let Some(cr) = chars.get(cur_pos) {
            if *cr == '#' {
                let _a = format!("{}", cr);
                cur_pos += 1;
                cur_pos =
                    find_comment_end_and_continue(chars, &vec![10 as char], cur_pos, to, true);
            } else if chars[cur_pos..cur_pos + 2].to_vec() == ['/', '/'] {
                cur_pos += 2;
                cur_pos =
                    find_comment_end_and_continue(chars, &vec![10 as char], cur_pos, to, true);
            } else if chars[cur_pos..cur_pos + 2].to_vec() == ['/', '*'] {
                cur_pos += 2;
                cur_pos = find_comment_end_and_continue(chars, &vec!['*', '/'], cur_pos, to, true);
            } else if chars[cur_pos..cur_pos + 6] == ['P', 'e', 'e', 'r', 's', ':']
                || chars[cur_pos..cur_pos + 8] == ['"', 'P', 'e', 'e', 'r', 's', '"', ':']
            {
                return cur_pos;
            }
        }
        cur_pos += 1;
    }

    cur_pos
}

fn find_comment_end_and_continue(
    chars: &Vec<char>,
    symbols: &Vec<char>,
    from: usize,
    to: usize,
    find_start: bool,
) -> usize {
    let mut cur_pos = from;
    let symbols_len = symbols.len();

    while cur_pos <= to {
        if chars[cur_pos..cur_pos + symbols_len].to_vec() == *symbols {
            if find_start {
                cur_pos += symbols_len;
                return cur_pos;
            } else {
                return cur_pos;
            }
        }
        cur_pos += 1;
    }

    cur_pos
}

fn find_end_of_peers_fragment(chars: &Vec<char>, from: usize, to: usize) -> usize {
    let mut cur_pos = from;

    let mut open_count: u8 = 0;
    let mut close_count: u8 = 0;

    while cur_pos <= to {
        if let Some(cr) = chars.get(cur_pos) {
            let cr_ = *cr;
            if cr_ == '#' {
                let _a = format!("{}", cr);
                cur_pos += 1;
                cur_pos =
                    find_comment_end_and_continue(chars, &vec![10 as char], cur_pos, to, false);
            } else if cr_ == '[' {
                open_count += 1;
            } else if cr_ == ']' {
                close_count += 1;
                if open_count > 0 && open_count == close_count {
                    return cur_pos;
                }
            } else if chars[cur_pos..cur_pos + 2].to_vec() == ['/', '/'] {
                cur_pos += 2;
                cur_pos =
                    find_comment_end_and_continue(chars, &vec![10 as char], cur_pos, to, false);
            } else if chars[cur_pos..cur_pos + 2].to_vec() == ['/', '*'] {
                cur_pos += 2;
                cur_pos = find_comment_end_and_continue(chars, &vec!['*', '/'], cur_pos, to, false);
            }
        }
        cur_pos += 1;
    }

    cur_pos
}
