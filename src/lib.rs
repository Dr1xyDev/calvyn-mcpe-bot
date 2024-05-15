pub mod client;
pub mod colors;
pub mod config;
pub mod crypto;
pub mod log;
pub mod proto;
pub mod raknet;

use client::{make_login_packet, Client, MTU};
use config::Device;
use log::{banner, bot, cmd_exit, err, start, user};
use crypto::AuthKey;
use proto::{ms_now, open_pkt_log, to_addr};
use rand::Rng;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ffi::CStr;
use std::io::{self, Write};
use std::net::UdpSocket;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};


fn setup_ctrlc_handler() {
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupted_flag = interrupted.clone();
    ctrlc::set_handler(move || {
        interrupted_flag.store(true, Ordering::SeqCst);
    })
    .ok();
}

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub protocol: u32,
}

#[no_mangle]
pub extern "C" fn RunBot(host_ptr: *const c_char, port: c_int, name_ptr: *const c_char) -> i32 {
    if host_ptr.is_null() {
        return 1;
    }

    let host = unsafe {
        match CStr::from_ptr(host_ptr).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => return 2,
        }
    };

    let name = if name_ptr.is_null() {
        "CalvynBot".to_string()
    } else {
        unsafe {
            match CStr::from_ptr(name_ptr).to_str() {
                Ok(s) => s.to_string(),
                Err(_) => "CalvynBot".to_string(),
            }
        }
    };

    let cfg = Config {
        host,
        port: port as u16,
        name,
        protocol: proto::VER,
    };

    match run_client(cfg) {
        Ok(_) => 0,
        Err(e) => {
            err(&e.to_string());
            3
        }
    }
}

pub fn run_client(cfg: Config) -> io::Result<()> {
    let server = to_addr(&cfg.host, cfg.port)?;
    banner();
    start(&cfg.host, cfg.port, &cfg.name);

    let auth = AuthKey::new();
    let dev = Device::load();
    let login_packet = make_login_packet(&cfg.host, cfg.port, &cfg.name, &auth, cfg.protocol, &dev);

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;

    let guid = ms_now() as i64 ^ 0x1130_0000_0000_0000u64 as i64;

    let mut client = Client {
        socket,
        server,
        auth,
        name: cfg.name.clone(),
        guid,
        srv_guid: 0,
        mtu: MTU,
        seq: 0,
        rel_idx: 0,
        ord_idx: 0,
        ord_idxs: HashMap::new(),
        ord_frames: BTreeMap::new(),
        split_id: 1,
        splits: HashMap::new(),
        pending: HashMap::new(),
        enc: None,
        bad_enc: 0,
        bad_batch: 0,
        resend_log_count: 0,
        in_cnt: HashMap::new(),
        out_cnt: HashMap::new(),
        text_line_counts: HashMap::new(),
        pack_ids: Vec::new(),
        packs: HashMap::new(),
        sent_have_all_packs: false,
        pending_chat: VecDeque::new(),
        joined: false,
        disconnected: false,
        sent_client_handshake: false,
        sent_chunk_radius: false,
        saw_start_game: false,
        first_chunk_at: None,
        last_chunk_at: None,
        entity_runtime_id: 0,
        pos: (0.0, 64.0, 0.0),
        yaw: 0.0,
        pitch: 0.0,
        dump: open_pkt_log("")?,
        raw_dump: open_pkt_log("")?,
        start: Instant::now(),
        chunk_radius: 8,
        spawn_fallback_ms: 8000,
        chat_interval_ms: 2500,
        chat_quiet_ms: 700,
        post_auth_delay_ms: 4500,
        chat_raw: false,
        chat_no_source: false,
        chat_source_name: true,
        chat_reliability: 3,
        command_step_for_slash: false,
        command_step_split_args: true,
        last_chat_sent_at: None,
        last_chat_wait_log_at: None,
        last_auth_transition_at: None,
        split_chunk: None,
        world_exporter: None,
        scan_radius_chunks: 0,
        scan_interval_ms: 180,
        protocol: cfg.protocol,
        scan_path: Vec::new(),
        scan_index: 0,
        scan_complete_at: None,
        scan_idle_finish_ms: 6000,
        finish_after_scan: false,
        dashboard: false,
        event_log: VecDeque::new(),
        status_text: "connecting".to_string(),
        last_text_line: String::new(),
        
        
        spawn_x: 0.0,
        spawn_z: 0.0,
        movement_phase: 0,
        last_movement_at: None,
        movement_enabled: true,
    };

    setup_ctrlc_handler();

    let mut rng = rand::thread_rng();

    client.ping()?;

    thread::sleep(Duration::from_millis(rng.gen_range(100..300)));

    client.connect()?;

    thread::sleep(Duration::from_millis(rng.gen_range(200..500)));

    client.handshake()?;

    thread::sleep(Duration::from_millis(rng.gen_range(300..800)));

    client.login(login_packet)?;

    thread::sleep(Duration::from_millis(rng.gen_range(500..1500)));

    let (tx, rx) = mpsc::channel::<String>();

    let exit_tx = tx.clone();
    thread::spawn(move || loop {
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let trimmed = line.trim().to_string();
            if trimmed == "/exit" || trimmed == "/quit" {
                cmd_exit();
                io::stdout().flush().ok();
                let _ = exit_tx.send("__EXIT__".to_string());
                break;
            }
            if !trimmed.is_empty() {
                user(&trimmed);
                io::stdout().flush().ok();
                if tx.send(trimmed).is_err() {
                    break;
                }
            }
        } else {
            break;
        }
    });

    client.run(0, Some(rx), None)?;

    client.disconnect()?;

    bot(&format!("завершён, был в сети {} сек.", client.start.elapsed().as_secs()));
    Ok(())
}
