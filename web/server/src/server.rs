// MIT License
//
// Copyright (c) 2019 Alasdair Armstrong
//
// Permission is hereby granted, free of charge, to any person
// obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
// BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
// ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};

use getopts::Options;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::task;
use tokio::sync::RwLock;
use warp::reject::Rejection;
use warp::Filter;

mod request;
use request::{Request, Response};

static WORKERS: AtomicUsize = AtomicUsize::new(0);
static MAX_WORKERS: usize = 10;

async fn spawn_worker_err(config: &Config, req: &Request) -> Option<String> {
    loop {
        let num = WORKERS.load(Ordering::SeqCst);
        if num < MAX_WORKERS && WORKERS.compare_and_swap(num, num + 1, Ordering::SeqCst) == num {
            break;
        }
        task::yield_now().await;
    }

    let mut command = Command::new(&config.worker);
    command.arg("--resources").arg(&config.resources).arg("--cache").arg(&config.cache);

    if let Some(value) = &config.ld_library_path {
        command.env("LD_LIBRARY_PATH", value);
    }

    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().ok()?;

    child.stdin.take().unwrap().write_all(&bincode::serialize(&req).ok()?).await.ok()?;

    let mut stdout = child.stdout.take().unwrap();

    let status = child.await.ok()?;

    let response = if status.success() {
        let mut response = Vec::new();
        stdout.read_to_end(&mut response).await.ok()?;
        String::from_utf8(response).ok()?
    } else {
        serde_json::to_string(&Response::InternalError).ok()?
    };

    let num = WORKERS.fetch_sub(1, Ordering::SeqCst);
    assert!(num != 0);

    Some(response)
}

async fn spawn_worker((config, req_cache, req): (&Config, &ReqCache, Request)) -> Result<String, Rejection> {
    let cached = {
        let cache = req_cache.read().await;
        cache.get(&req).map(String::to_owned)
    };

    match cached {
        Some(response) => Ok(response),
        None => match spawn_worker_err(config, &req).await {
            Some(response) => {
                let mut cache = req_cache.write().await;
                cache.insert(req, response.clone());
                Ok(response)
            }
            None => Err(warp::reject::reject()),
        }
    }
}

#[derive(Clone)]
struct Config {
    worker: PathBuf,
    dist: PathBuf,
    resources: PathBuf,
    cache: PathBuf,
    address: SocketAddr,
    ld_library_path: Option<String>,
    tls_cert: Option<String>,
    tls_key: Option<String>,
}

fn get_config() -> &'static Config {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = Options::new();
    opts.reqopt("", "worker", "path to worker process", "<path>");
    opts.reqopt("", "dist", "path to static files", "<path>");
    opts.reqopt("", "resources", "path to resource files", "<path>");
    opts.reqopt("", "cache", "path to a cache directory", "<path>");
    opts.reqopt("", "address", "socket address to run server on", "<address:port>");
    opts.optopt("", "ld-library-path", "LD_LIBRARY_PATH for worker", "<path>");
    opts.optopt("", "tls-cert", "TLS cert file", "<path>");
    opts.optopt("", "tls-key", "TLS key file", "<path>");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error: {}\n{}", e, opts.usage("islaweb-server <options>"));
            std::process::exit(1)
        }
    };

    Box::leak(Box::new(Config {
        worker: PathBuf::from(matches.opt_str("worker").unwrap()),
        dist: PathBuf::from(matches.opt_str("dist").unwrap()),
        resources: PathBuf::from(matches.opt_str("resources").unwrap()),
        cache: PathBuf::from(matches.opt_str("cache").unwrap()),
        address: matches.opt_str("address").unwrap().parse().unwrap(),
        ld_library_path: matches.opt_str("ld-library-path"),
        tls_cert: matches.opt_str("tls-cert"),
        tls_key: matches.opt_str("tls-key"),
    }))
}

type ReqCache = RwLock<HashMap<Request, String>>;

fn create_cache() -> &'static ReqCache {
    Box::leak(Box::new(RwLock::new(HashMap::new())))
}

#[tokio::main]
async fn main() {
    let config = get_config();
    let req_cache = create_cache();

    let dist = warp::filters::query::query::<Request>()
        .map(move |req| (config, req_cache, req))
        .and_then(spawn_worker)
        .or(warp::fs::dir(&config.dist));

    if cfg!(feature = "https") {
        warp::serve(dist)
            .tls()
            .cert_path(config.tls_cert.as_ref().unwrap())
            .key_path(config.tls_key.as_ref().unwrap())
            .run(config.address)
            .await
    } else {
        warp::serve(dist).run(config.address).await
    }
}
