use std::{
    fmt::Write as FmtWrite,
    io::Write,
    net::{TcpListener, ToSocketAddrs},
};

use thingbuf::{mpsc::blocking::StaticReceiver, recycling::WithCapacity};

use crate::idf_libs::log_redirection::log_crate_shenanigans::EspStdout;

pub fn remote_log_server(
    addr: impl ToSocketAddrs,
    rx: StaticReceiver<String, WithCapacity>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr)?;

    'conn_loop: for connection in listener.incoming() {
        let Ok(mut stream) = connection else {
            log::error!("accepting connection failed");
            continue;
        };

        let addr = stream
            .peer_addr()
            .map(|v| v.to_string())
            .unwrap_or_else(|_| String::from("[Unknown Socket]"));

        loop {
            let Some(mut log) = rx.recv_ref() else {
                return Ok(());
            };

            let from_rust = log.pop() == Some('R');

            if from_rust {
                let mut stdout = EspStdout::new();
                stdout.write_str(&log);
            }

            if let Err(e) = stream.write_all(log.as_bytes()) {
                log::error!("failed logging to socket {addr}: {e}. closing connection");
                match stream.shutdown(std::net::Shutdown::Both) {
                    Ok(_) => log::info!("succesfully shutdown {addr}"),
                    Err(e) => log::error!("failed to shudown {addr}: {e}"),
                };

                drop(log);
                continue 'conn_loop;
            }
        }
    }

    Ok(())
}
