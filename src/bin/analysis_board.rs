//! Local analysis-board launcher; no external services or model files.
use fusion_engine::{
    board::Board,
    eval::EvalWeights,
    header::Piece,
    search::{search, SearchRequest},
    search_config::SearchConfig,
    state::GameState,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    rows: Vec<u16>,
    current: String,
    hold: Option<String>,
    queue: Vec<String>,
    combo: u32,
    b2b: u8,
    pending: u8,
}
fn piece(s: &str) -> Result<Piece, String> {
    match s {
        "I" => Ok(Piece::I),
        "O" => Ok(Piece::O),
        "T" => Ok(Piece::T),
        "S" => Ok(Piece::S),
        "Z" => Ok(Piece::Z),
        "J" => Ok(Piece::J),
        "L" => Ok(Piece::L),
        _ => Err(format!("Invalid piece: {s}")),
    }
}
fn analyze(body: &[u8]) -> Result<Value, String> {
    let p: Position = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if p.rows.len() != 40 || p.rows.iter().any(|r| *r > 1023) {
        return Err("Expected 40 rows of 10 cells".into());
    }
    if p.queue.is_empty() || p.queue.len() > 32 || p.combo > 100 {
        return Err("Supply 1–32 next pieces; combo must be at most 100".into());
    }
    if p.rows.iter().any(|r| *r == 1023) {
        return Err(
            "A full row would already have cleared. Erase a cell from each full row.".into(),
        );
    }
    let mut rows = [0u16; 40];
    rows.copy_from_slice(&p.rows);
    let mut state = GameState::new(
        Board::from_rows(rows),
        piece(&p.current)?,
        p.queue.iter().map(|s| piece(s)).collect::<Result<_, _>>()?,
    );
    state.hold = p.hold.as_deref().map(piece).transpose()?;
    state.combo = p.combo;
    state.b2b = p.b2b;
    state.pending_garbage = p.pending;
    let mut config = SearchConfig {
        depth: 8,
        beam_width: 300,
        time_budget_ms: Some(1500),
        extend_queue_7bag: false,
        ..SearchConfig::default()
    };
    config.attack_config.pc_garbage = 0;
    config.attack_config.pc_b2b = 0;
    let weights = EvalWeights::default();
    let full = search(
        &state,
        &SearchRequest {
            config: &config,
            weights: &weights,
            runtime: None,
            forced_root_move: None,
        },
    )
    .ok_or("No legal continuation found. Lower the stack or change the pieces.")?;
    let best = &full.best;
    let m = best.best_move;
    let mut cells = vec![[m.x(), m.y()]];
    for i in 0..3 {
        let c = m.cells()[i];
        cells.push([m.x() + c.x as i32, m.y() + c.y as i32]);
    }
    let mech = state.board.lock(&m);
    let transition =
        state
            .chain_state()
            .advance_lock(&m, &mech, best.hold_used, false, &config.attack_config);
    let mut queue = p.queue.clone();
    let hold = if best.hold_used {
        Some(p.current.clone())
    } else {
        p.hold.clone()
    };
    if best.hold_used && p.hold.is_none() {
        queue.remove(0);
    }
    let next = if queue.is_empty() {
        None
    } else {
        Some(queue.remove(0))
    };
    Ok(
        json!({"piece":format!("{:?}",m.piece()),"rotation":m.rotation() as u8,"hold_used":best.hold_used,"cells":cells,
        "score":best.score,"after":state.board.rows.to_vec(),"lines":mech.lines_cleared,
        "next":next,"queue":queue,"hold":hold,"combo":transition.chain.combo,"b2b":transition.chain.b2b,
        "pending":transition.chain.pending_garbage,"path":best.pv.iter().map(|m|format!("{:?}",m.piece())).collect::<Vec<_>>() }),
    )
}
fn reply(stream: &mut TcpStream, code: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {code}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n",body.len())?;
    stream.write_all(body)
}
fn handle(mut stream: TcpStream, host: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let end = loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        data.extend_from_slice(&buf[..n]);
        if data.len() > 65536 {
            return reply(
                &mut stream,
                "413 Payload Too Large",
                "text/plain",
                b"Request too large",
            );
        }
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let headers = String::from_utf8_lossy(&data[..end]).to_string();
    let mut lines = headers.lines();
    let first = lines.next().unwrap_or("");
    let mut length = 0usize;
    let mut valid_host = false;
    let mut valid_origin = true;
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            match key.to_ascii_lowercase().as_str() {
                "host" => valid_host = value == host,
                "origin" => valid_origin = value == format!("http://{host}"),
                "content-length" => length = value.parse().unwrap_or(65537),
                _ => (),
            }
        }
    }
    if !valid_host || !valid_origin {
        return reply(
            &mut stream,
            "403 Forbidden",
            "text/plain",
            b"Only local same-origin requests are accepted",
        );
    }
    if first == "GET / HTTP/1.1" {
        return reply(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            include_bytes!("../../analysis-board/index.html"),
        );
    }
    if first != "POST /analyze HTTP/1.1" {
        return reply(&mut stream, "404 Not Found", "text/plain", b"Not found");
    }
    if length > 32768 {
        return reply(
            &mut stream,
            "413 Payload Too Large",
            "text/plain",
            b"Request too large",
        );
    }
    while data.len() < end + length {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        data.extend_from_slice(&buf[..n]);
    }
    let result = std::panic::catch_unwind(|| analyze(&data[end..end + length]));
    let (code, value) = match result {
        Ok(Ok(v)) => ("200 OK", v),
        Ok(Err(e)) => ("400 Bad Request", json!({"error":e})),
        Err(_) => (
            "500 Internal Server Error",
            json!({"error":"Engine could not analyze this position"}),
        ),
    };
    reply(
        &mut stream,
        code,
        "application/json",
        value.to_string().as_bytes(),
    )
}
fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let host = listener.local_addr()?.to_string();
    let url = format!("http://{host}/");
    println!("Fusion analysis board: {url}\nLeave this window open. Press Ctrl+C to stop.");
    #[cfg(target_os = "windows")]
    {
        if !std::env::args().any(|a| a == "--no-browser") {
            let _ = std::process::Command::new("rundll32.exe")
                .args(["url.dll,FileProtocolHandler", &url])
                .spawn();
        }
    }
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle(s, &host) {
                    eprintln!("Request: {e}");
                }
            }
            Err(e) => eprintln!("{e}"),
        }
    }
    Ok(())
}
