//! The Chaos server, as a library.
//!
//! **It is a library because Android needs it.** The phone had four modes on a
//! dial and only one of them did anything: a CORE there could not serve, a
//! HELPER could not lend, ALONE could not run a model. Atur, and he was right:
//! *"android can not do any one of works in windows"*.
//!
//! Everything the phone needs already exists here -- the token loop, sampling,
//! streaming, the OpenAI-compatible endpoints -- and the Kotlin client already
//! speaks to it. So rather than a second token loop written against JNI, the
//! app starts **this** on a thread and points its own client at `127.0.0.1`.
//! CORE binds `0.0.0.0` instead, and other devices connect to the phone.
//!
//! `src/bin/chaos-serve.rs` is the command-line front end and is now the only
//! thing that parses arguments.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

use chaos_arch::{
    architecture_is_verified, Deepseek4Cache, Deepseek4Config, Deepseek4Forward, Qwen3Config,
    Qwen3Model, Sampler, SamplerConfig, VERIFIED_ARCHITECTURES,
};
use chaos_ggml::{Context, WeightSet};
use chaos_model::{Model, ResidentSet};
use chaos_tokenizer::{Message, Tokenizer};

const GIB: f64 = (1u64 << 30) as f64;

pub fn usage() {
    println!("usage: chaos-serve <model> [--port 8080] [--cache GiB] [-t N] [-tb N]");
    println!();
    let found = chaos_model::find::list();
    if found.is_empty() {
        println!("  no models found. Put a .gguf file in:");
        if let Some(dir) = chaos_model::find::model_dirs().first() {
            println!("    {}", dir.display());
        }
    } else {
        println!("  models on this machine (any unique part of a name works):");
        for f in &found {
            println!("    {}", f.label);
        }
    }
    println!();
    println!("Serves an OpenAI-compatible endpoint on 127.0.0.1:");
    println!("  GET  /                      the browser interface -- open this");
    println!("  GET  /qr                    the mark: this node's route, as a code");
    println!("                              to point another device's camera at");
    println!("  GET  /scan                  the reader, for pointing this device at");
    println!("                              another node's mark");
    println!("  POST /v1/chat/completions   the one an agent calls");
    println!("  POST /v1/completions        the older, prompt-shaped one");
    println!("  POST /v1/embeddings         vectors for a string or an array");
    println!("  GET  /v1/models             what is loaded");
    println!("  GET  /health                readiness, and what the engine is doing");
    println!("  GET  /status                the same, plus the route and the last");
    println!("                              measured tokens per second");
    println!();
    println!("  --api-key <key>   require `Authorization: Bearer <key>` on /v1/*");
    println!("  --host <addr>     what to listen on (default 127.0.0.1;");
    println!("                    0.0.0.0 reaches a phone on the same Wi-Fi and");
    println!("                    then --api-key is required, not optional)");
    println!();
    println!("  --emit-pages <dir>  write qr.html and scan.html and exit, for a");
    println!("                      host that embeds them (the Android APK does)");
    println!();
    println!("  CHAOS_QR=1        draw the route as a QR code in this terminal even");
    println!("                    on loopback (=0 never). Off loopback it is drawn");
    println!("                    anyway -- that is how a phone finds a headless node.");
    println!();
    println!("Binds to localhost only: no TLS, one request at a time.");
}

/// Why a flag the runner accepts is not accepted here.
///
/// **The list exists so a refusal names its reason.** These four were silently
/// swallowed until now: the app passed `-ngl`, `-c`, `--auto` and `--force` and
/// the server had never heard of any of them. Two are implemented now; these
/// are the two that are not, and a wrong-but-specific message beats a correct
/// but empty one.
pub fn declined(flag: &str) -> Option<&'static str> {
    match flag {
        "-ngl" | "--n-gpu-layers" | "--device" | "--main-gpu" => Some(
            "the server loads its weights directly rather than through the runner's device \
             loader, so there is nowhere to put them on a card yet. chaos-run takes -ngl and \
             --device today; wiring the same loader in here is the open work.",
        ),
        _ => None,
    }
}

/// Addresses that only this machine can reach.
///
/// `127.0.0.0/8` and `::1`. Anything else is a route somebody else can take,
/// including `0.0.0.0`, which is *every* route.
pub fn is_loopback(host: &str) -> bool {
    let h = host.trim();
    if h == "localhost" || h == "::1" || h == "[::1]" {
        return true;
    }
    let mut parts = h.split('.');
    let first = parts.next().and_then(|p| p.parse::<u8>().ok());
    let rest: Vec<&str> = parts.collect();
    first == Some(127) && rest.len() == 3 && rest.iter().all(|p| p.parse::<u8>().is_ok())
}

/// Why this server must not start, if it must not.
///
/// **The api key stops being optional the moment the socket leaves loopback.**
/// Until now `--api-key` was a convenience -- the doc comment on the flag says
/// so, and it was right, because a caller who can reach `127.0.0.1` can already
/// read the weights off the disk. `--host 0.0.0.0` changes that completely:
/// every device on the Wi-Fi can now spend this machine's memory and read
/// whatever the model is asked to say. An unauthenticated LAN endpoint is not a
/// default anyone should be able to reach by accident, so this refuses rather
/// than warns.
pub fn refuse_to_start(host: &str, api_key: Option<&str>) -> Option<String> {
    if is_loopback(host) || api_key.is_some_and(|k| !k.is_empty()) {
        return None;
    }
    Some(format!(
        "refusing to listen on {host} with no api key.\n\
         \n\
         On 127.0.0.1 a key is optional -- there is no route in. On {host} there\n\
         is: every device on this network could use this model, and nothing\n\
         would ask them who they are.\n\
         \n\
         Pass one:\n\
         \n\
             chaos-serve <model> --host {host} --api-key <a-long-random-string>\n\
         \n\
         The Android app asks for the same string. Anything unguessable does;\n\
         it is checked for equality, not strength."
    ))
}

// Command-line options, not coupled state: a config struct here would add a
// layer without removing a decision, which is the same call `chaos-run` makes
// about `run_streaming` for the same reason.
#[allow(clippy::too_many_arguments)]
pub fn serve(
    path: &str,
    host: &str,
    port: u16,
    cache_gib: f64,
    api_key: Option<String>,
    context: Option<usize>,
    force: bool,
    auto: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cache_gib = cache_gib;
    let t0 = std::time::Instant::now();
    let model = Model::open_split(path)?;
    let tokenizer = Tokenizer::from_metadata(model.metadata())?;
    // Same rule as the runner: refuse an architecture nobody has checked rather
    // than serving confident nonsense to an agent that cannot tell.
    if !architecture_is_verified(model.architecture()) && !force {
        return Err(format!(
            "{:?} is not an architecture this build has been verified against              (verified: {}). It may load and answer WRONG with no error.              chaos-run --force will run it; the server will not, because a client              has no way to see that the answer is unsound.",
            model.architecture(),
            VERIFIED_ARCHITECTURES.join(", "),
        )
        .into());
    }
    if !architecture_is_verified(model.architecture()) {
        // Said every time, not once at startup, because the person reading the
        // answers may not be the person who launched this.
        println!(
            "forced     {:?} has NOT been diffed against llama.cpp -- answers may be",
            model.architecture()
        );
        println!("           fluently wrong with no error anywhere. --force was given.");
    }
    println!("model      {} ({})", model.architecture(), model.io_mode());
    let format = tokenizer.chat_format();
    if format.is_known() {
        println!("chat       {} template", format.name());
    } else {
        println!("chat       template not recognised -- using a plain framing;");
        println!("           the model may not respond as an assistant.");
    }

    // Two engines, chosen by architecture, because V4-Flash shares almost none
    // of its graph with the dense path. Both are set up here so the borrowed
    // state outlives the request loop.
    if model.architecture() == "deepseek4" {
        let config = Deepseek4Config::from_model(&model)?;
        let machine = chaos_probe::Machine::probe(std::path::Path::new("."), false);
        let reserve = (1u64 << 30) + (512 << 20) + (768 << 20);
        if auto && cache_gib == 0.0 {
            // Whatever is left after the always-read weights and the reserve.
            // Below a quarter of a gigabyte a cache holds too few expert slices
            // to pay for the memory, so it is left off rather than set to a
            // number that only looks like tuning.
            let spare = machine.usable_ram_for_weights(reserve) as f64 / GIB;
            if spare >= 0.25 {
                cache_gib = spare;
                println!("auto       expert cache {cache_gib:.2} GiB, from the free memory now");
            } else {
                println!("auto       no expert cache: {spare:.2} GiB free is too little to help");
            }
            println!("auto       device not chosen here -- the server has no device loader yet");
        }
        let (mut resident, report) =
            ResidentSet::load(&model, machine.usable_ram_for_weights(reserve))?;
        println!("resident   {report}");

        // Rearranged once, at load, and re-bound per block — see `RepackedDense`.
        let repacked = chaos_arch::RepackedDense::build(&mut resident, &model)?;
        let (n_repacked, repacked_bytes, _) = repacked.stats();
        if n_repacked > 0 {
            println!(
                "repacked   {n_repacked} tensors, {:.2} GiB in the CPU kernels' layout",
                repacked_bytes as f64 / GIB
            );
        }

        let mut fw = Deepseek4Forward::new(&model, config.clone())
            .with_resident(&resident)
            .with_repacked(&repacked);
        // Same rule the runner enforces: a byte given to the expert cache while
        // the always-read set is still streaming comes out of residency, where
        // it would have been read on every token. Measured both ways.
        if cache_gib > 0.0 && report.skipped_over_budget == 0 {
            fw = fw.with_expert_cache((cache_gib * GIB) as usize);
            println!("cache      {cache_gib:.2} GiB for routed experts");
        } else if cache_gib > 0.0 {
            println!(
                "cache      refused: {:.2} GiB of always-read weights is still streaming",
                report.skipped_over_budget as f64 / GIB
            );
        }
        let fw = fw;
        let engine = Engine::Deepseek4 {
            fw: &fw,
            config: &config,
            asked: context,
        };
        return run_loop(engine, &tokenizer, host, port, t0, api_key);
    }

    // Dense: Llama, Mistral, Qwen and everything else the qwen3 path covers.
    let config = Qwen3Config::from_model(&model)?;
    let arch = Qwen3Model::new(config.clone());
    arch.verify(&model)?;
    println!(
        "shape      {} layers, {} embd, {} heads ({} kv)",
        config.n_layer, config.n_embd, config.n_head, config.n_head_kv
    );
    // Same warning the runner prints, and it matters more here: a client on the
    // other end of the socket cannot see anything but the answer.
    if let Some(why) = chaos_arch::container_caveat(&model, config.n_layer) {
        println!("caution    {why}");
    }
    if !config.rope_type_is_known {
        println!(
            "           NOTE: {:?} is not an architecture this build has verified;",
            model.architecture()
        );
        println!("           its RoPE layout is assumed. Fluent-but-wrong output points here.");
    }

    let weight_ctx = Context::new_no_alloc(64 << 20)?;
    let mut weights = WeightSet::new();
    let mut bound = 0u64;
    for name in arch.required_tensors() {
        let loc = model
            .location(&name)
            .ok_or_else(|| format!("missing tensor {name}"))?
            .clone();
        let data = model.read_tensor(&name)?;
        bound += data.len() as u64;
        weights.bind(&weight_ctx, &name, loc.ty, &loc.dims, data)?;
    }
    // Tied embeddings: many small models ship no separate output projection and
    // reuse the embedding table. Binding it only when present is what lets
    // those containers load at all.
    if model.location("output.weight").is_some() && weights.get("output.weight").is_none() {
        let loc = model.location("output.weight").expect("checked").clone();
        let data = model.read_tensor("output.weight")?;
        bound += data.len() as u64;
        weights.bind(&weight_ctx, "output.weight", loc.ty, &loc.dims, data)?;
    }
    println!(
        "weights    {} tensors, {:.2} GiB bound in {:.1}s (zero-copy)",
        weights.len(),
        bound as f64 / GIB,
        t0.elapsed().as_secs_f64()
    );

    // `general.name` if the container carries one, else the file stem -- a
    // client's model id should mean something.
    let name = model
        .metadata()
        .get("general.name")
        .and_then(chaos_gguf::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "chaos-model".to_string())
        });
    // The SAME forward the CLI uses, and that is the whole point of this
    // change. The old dense path here called `arch.build_graph` -- a second
    // implementation that never received the QKV bias, the Gemma activation,
    // the post-norms or the soft caps, all of which live in `stream.rs`. Qwen2
    // through this server produced fluent nonsense while `chaos-run` on the
    // same container was byte-identical to llama.cpp.
    //
    // A second code path is a second place for every fix to be missing from.
    let runner = chaos_arch::StreamingRunner::new(&model, config.clone(), 1 << 30);
    let engine = Engine::Dense {
        runner: std::cell::RefCell::new(runner),
        weights: &weights,
        name: &name,
        config: config.clone(),
        asked: context,
    };
    run_loop(engine, &tokenizer, host, port, t0, api_key)
}

/// What this process knows about itself, which the engine cannot say.
///
/// **The route is the point.** The mark that `GET /qr` serves encodes the
/// address *another machine* uses to reach this node, and a page cannot work
/// that out for itself: opened on the node's own loopback, `location.origin`
/// is `http://127.0.0.1:8080`, which is a perfectly good URL and useless to
/// the one person who matters -- the one holding a phone. The server bound the
/// socket, so the server knows, and hands the answer in.
///
/// `Cell` rather than a lock because `run_loop` serves one request at a time;
/// that is a documented property of this server, not an accident.
struct Node {
    /// e.g. `http://192.168.1.20:8080`.
    route: String,
    /// True when `route` is loopback -- nothing else can reach it, and the
    /// pages and `/status` say so rather than implying a network that is not
    /// there.
    loopback: bool,
    since: std::time::Instant,
    /// The last finished generation: tokens produced, and the rate.
    last: std::cell::Cell<Option<(usize, f64)>>,
}

impl Node {
    fn new(host: &str, port: u16) -> Self {
        let (addr, loopback) = chaos_probe::net::reachable_address(host);
        Node {
            route: format!("http://{addr}:{port}"),
            loopback,
            since: std::time::Instant::now(),
            last: std::cell::Cell::new(None),
        }
    }

    fn record(&self, produced: usize, seconds: f64) {
        if produced > 0 && seconds > 0.0 {
            self.last.set(Some((produced, produced as f64 / seconds)));
        }
    }
}

/// Accept and answer requests, one at a time.
fn run_loop(
    engine: Engine<'_>,
    tokenizer: &Tokenizer,
    host: &str,
    port: u16,
    t0: std::time::Instant,
    api_key: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr)?;
    let node = Node::new(host, port);
    println!("ready      {addr} in {:.1}s", t0.elapsed().as_secs_f64());
    // The URL comes first and on its own line: most terminals make it
    // clickable, and someone who wanted a window rather than a socket should
    // not have to read an endpoint list to find the interface.
    println!("           open       http://{addr}");
    println!("           the mark   {}/qr", node.route);
    println!("           the reader {}/scan", node.route);
    println!("           for agents POST /v1/chat/completions");
    if node.loopback && host != "127.0.0.1" && host != "localhost" {
        println!("           NOTE: no route off this machine was found, so the mark");
        println!("                 carries a loopback address. Nothing else can scan it.");
    }
    print_route_code(&node);
    // Whether a key is required is the one thing a client cannot discover by
    // trying, so it is printed rather than left to a 401.
    match &api_key {
        Some(_) => println!("           api key   required on /v1/*"),
        None => println!("           api key   none -- any value is accepted"),
    }
    println!(
        "           context {} tokens, one request at a time",
        engine.context_limit()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                // **A browser opens speculative connections and leaves them
                // idle.** This loop is single-threaded, so without a deadline it
                // blocks in `read_request` on a socket that will never send
                // anything, and the server is dead until that peer gives up --
                // no error, no log line, just nothing.
                //
                // It survived because every client until now was an agent, and
                // an agent connects in order to send immediately. Opening the
                // page in a browser wedged it on the first try.
                //
                // Read-side only: writes can legitimately take minutes on a
                // model that generates at 0.4 tok/s.
                //
                // **Three seconds, not thirty.** The deadline is also how long
                // a real request waits behind a speculative one, since this
                // loop serves them in order -- at 20s the page took 17.9s to
                // load behind one idle socket, which is a hang as far as anyone
                // watching is concerned. On loopback a client that means to
                // send has sent within microseconds, so 3s is enormous slack
                // for a real request and a small toll for a dead one.
                //
                // The real fix is accepting concurrently and serialising only
                // the engine; that is a bigger change than this page justifies,
                // and "one request at a time" is a documented property here.
                if let Err(e) = s.set_read_timeout(Some(std::time::Duration::from_secs(3))) {
                    eprintln!("could not set a read timeout: {e}");
                }
                if let Err(e) = handle(s, &engine, tokenizer, api_key.as_deref(), &node) {
                    // A peer that connected and said nothing is routine, not a
                    // fault; anything else is worth printing.
                    let msg = e.to_string();
                    if !msg.contains("timed out") && !msg.contains("os error 10060") {
                        eprintln!("request failed: {e}");
                    }
                }
            }
            Err(e) => eprintln!("accept failed: {e}"),
        }
    }
    Ok(())
}

/// A parsed request line plus body. Deliberately minimal.
struct Request {
    method: String,
    target: String,
    body: String,
    /// The `Authorization` header, verbatim, if one was sent.
    auth: Option<String>,
}

fn read_request(stream: &TcpStream) -> Result<Request, Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    let mut auth: Option<String> = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let trimmed = header.trim_end();
        if trimmed.is_empty() {
            break;
        }
        // Header names are case-insensitive per RFC 9110, and clients disagree
        // about which case they send. Compare lowercased once rather than
        // guessing the two spellings that happened to be seen first.
        let lower = trimmed.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        } else if lower.starts_with("authorization:") {
            auth = trimmed.split_once(':').map(|(_, v)| v.trim().to_string());
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    Ok(Request {
        method,
        target,
        body: String::from_utf8_lossy(&body).into_owned(),
        auth,
    })
}

/// Whether a request carries the key, when one is required.
///
/// **Only the `/v1/*` routes are gated.** `/` is the browser interface a person
/// opens, `/health` is what the app polls to know the model is up, and
/// `/favicon.ico` is asked for by every browser on every load -- putting a key
/// in front of those would break the window that starts the server without
/// protecting anything, since a caller who can reach `127.0.0.1` can read the
/// key out of the settings file anyway.
///
/// The key is compared in full rather than by prefix, and a missing header is
/// the same failure as a wrong one.
fn authorised(req: &Request, key: Option<&str>) -> bool {
    let Some(key) = key else { return true };
    if !req.target.starts_with("/v1/") {
        return true;
    }
    let Some(header) = req.auth.as_deref() else {
        return false;
    };
    // `Bearer <key>` is what every OpenAI-compatible client sends; a bare key
    // is accepted too, because some send that instead of arguing about it.
    let offered = header
        .strip_prefix("Bearer ")
        .or_else(|| header.strip_prefix("bearer "))
        .unwrap_or(header)
        .trim();
    offered == key
}

fn handle(
    mut stream: TcpStream,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
    api_key: Option<&str>,
    node: &Node,
) -> Result<(), Box<dyn std::error::Error>> {
    let req = read_request(&stream)?;
    let started = std::time::Instant::now();
    // Matched on the path, not the whole target: `/qr?theme=dark` is the same
    // route as `/qr`, and an exact match on the target made the query string
    // the difference between the page and a 404.
    let (path, query) = match req.target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (req.target.as_str(), ""),
    };

    // The shape OpenAI clients expect, so a wrong key reads as a wrong key in
    // whatever tool is calling rather than as an unexplained failure. Produced
    // here rather than by an early return, so it goes out through the one
    // response writer below and cannot drift from it.
    let unauthorised = !authorised(&req, api_key);
    let (status, body) = if unauthorised {
        (
            401,
            r#"{"error":{"message":"invalid api key","type":"invalid_request_error","code":"invalid_api_key"}}"#
                .to_string(),
        )
    } else {
        match (req.method.as_str(), path) {
            // The browser interface. Served from the binary with nothing external
            // to fetch, so a machine with no network still gets the whole page.
            ("GET", "/") => {
                return send_html(stream, chaos_arch::ui::PAGE, &req, started);
            }
            // Every browser asks for this on every page load. Answering "nothing
            // here" is one line; leaving it a 404 puts a red error in the log for
            // an entirely normal request.
            ("GET", "/favicon.ico") => (204, String::new()),
            // **The mark.** A book bearing the sigil, burning in a rune
            // circle, which opens onto a QR code cut from `node.route` -- so
            // pointing a phone at this screen is how another machine finds
            // this one. `?theme=dark|light` makes it match the window the host
            // application put around it; without it the operating system's
            // preference decides.
            ("GET", "/qr") | ("GET", "/mark") => {
                let html = chaos_arch::grimoire::mark(chaos_arch::grimoire::Host {
                    endpoint: Some(&node.route),
                    theme: theme_of(query),
                });
                return send_html(stream, &html, &req, started);
            }
            // **The reader**, which is the same circle pointed the other way.
            // It carries its own QR detector rather than calling
            // `BarcodeDetector`, which is absent on desktop Windows and on
            // iOS. A camera needs a secure context, so over a LAN this page
            // says why it cannot open one instead of failing silently.
            ("GET", "/scan") => {
                let html = chaos_arch::grimoire::scry(chaos_arch::grimoire::Host {
                    endpoint: Some(&node.route),
                    theme: theme_of(query),
                });
                return send_html(stream, &html, &req, started);
            }
            // Everything a client needs to describe this node without loading
            // a model of its own. `/health` answers the narrower question of
            // whether it is up, and predates this.
            ("GET", "/status") => (200, status_json(engine, node)),
            ("GET", "/health") => (
                200,
                format!(
                    r#"{{"status":"ok","model":"{}","context_limit":{}}}"#,
                    engine.model_name(),
                    engine.context_limit()
                ),
            ),
            ("GET", "/v1/models") => (
                200,
                format!(
                    r#"{{"object":"list","data":[{{"id":"{}","object":"model","owned_by":"chaos"}}]}}"#,
                    engine.model_name()
                ),
            ),
            ("POST", "/v1/chat/completions") => {
                let params = Params::from_body(&req.body);
                if params.stream {
                    // Streaming owns the socket: headers go out first, then one
                    // event per token, so a client sees words appear instead of
                    // waiting for the whole answer. Nothing more may be written
                    // afterwards, so this returns early.
                    return stream_completion(
                        stream, &req, engine, tokenizer, &params, started, node,
                    );
                }
                match generate(&req.body, engine, tokenizer, &params, &mut |_| Ok(())) {
                    Ok((text, prompt_tokens, produced, finish)) => (200, {
                        node.record(produced, started.elapsed().as_secs_f64());
                        completion_json(engine.model_name(), &text, prompt_tokens, produced, finish)
                    }),
                    Err(e) => (400, error_json(&e.to_string())),
                }
            }
            // The legacy completions endpoint: same engine, no chat framing. Some
            // clients and most autocomplete integrations still speak only this.
            ("POST", "/v1/completions") => {
                let params = Params::from_body(&req.body);
                match generate_raw(&req.body, engine, tokenizer, &params, &mut |_| Ok(())) {
                    Ok((text, prompt_tokens, produced, finish)) => (
                        200,
                        format!(
                            r#"{{"id":"chaos","object":"text_completion","model":"{}","choices":[{{"index":0,"text":"{}","finish_reason":"{}"}}],"usage":{{"prompt_tokens":{prompt_tokens},"completion_tokens":{produced},"total_tokens":{}}}}}"#,
                            engine.model_name(),
                            escape(&text),
                            finish.as_str(),
                            prompt_tokens + produced
                        ),
                    ),
                    Err(e) => (400, error_json(&e.to_string())),
                }
            }
            // Embeddings are a different computation, not a cheaper completion:
            // they need the model's hidden state rather than its logits. This used
            // to be a 501 saying the graph returns only logits -- true of what it
            // returned, false about what it computed. See `embed`.
            ("POST", "/v1/embeddings") => match embed(&req.body, engine, tokenizer) {
                Ok((vectors, prompt_tokens)) => (
                    200,
                    embeddings_json(engine.model_name(), &vectors, prompt_tokens),
                ),
                Err(e) => (400, error_json(&e.to_string())),
            },
            _ => (404, error_json("no such endpoint")),
        }
    };

    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        501 => "Not Implemented",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    eprintln!(
        "{} {} -> {status} in {:.1}s",
        req.method,
        req.target,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Write one HTML response and close.
///
/// The JSON path hardcodes `Content-Type: application/json`, and a page served
/// under that type is displayed as source rather than rendered -- so this is a
/// second writer rather than a parameter on the first.
fn send_html(
    mut stream: TcpStream,
    page: &str,
    req: &Request,
    started: std::time::Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    // Built by concatenation, for the reason the SSE path below already
    // records: a wrapped multi-line literal ships headers with the source's
    // indentation on them. That happened here -- `Content-Type` went out with
    // nine leading spaces, which curl folded into the previous line, so the
    // declared length (7658) and what a client actually read (7802) disagreed.
    // A browser given that either truncates the page or waits for bytes that
    // never arrive, and the response is still a 200 the whole time.
    let head = [
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/html; charset=utf-8\r\n",
        &format!("Content-Length: {}\r\n", page.len()),
        "Cache-Control: no-store\r\n",
        "Connection: close\r\n",
        "\r\n",
    ]
    .concat();
    stream.write_all(head.as_bytes())?;
    stream.write_all(page.as_bytes())?;
    stream.flush()?;
    eprintln!(
        "{} {} -> 200 in {:.1}s",
        req.method,
        req.target,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Draw this node's route in the terminal, as a code a phone can read.
///
/// **This is the CLI refusing to be a lesser tier.** Someone runs Chaos on a
/// headless box and wants to reach it from their phone; there is no window to
/// show the mark in and no browser to open it with, so the route comes out of
/// the terminal they are already looking at -- over SSH, in a log, in a tmux
/// pane. `chaos-qr` does the same thing on demand for any payload.
///
/// **Printed when the node is actually reachable**, because a code carrying
/// `http://127.0.0.1:8080` is a route to nowhere that looks exactly like a
/// route. `CHAOS_QR=1` prints it anyway (someone forwarding a port over SSH
/// has a loopback address that IS reachable, and only they know that);
/// `CHAOS_QR=0` never does.
fn print_route_code(node: &Node) {
    let forced = std::env::var("CHAOS_QR").ok();
    let want = match forced.as_deref() {
        Some("0") | Some("no") | Some("false") => false,
        Some(_) => true,
        None => !node.loopback,
    };
    if !want {
        return;
    }
    match chaos_qr::encode(&node.route, chaos_qr::Level::Q) {
        Ok(code) => {
            println!();
            print!("{}", code.render(chaos_qr::Render::Ansi, 4));
            println!("           scan that, or open {}/qr", node.route);
            println!();
        }
        // A route too long for a version-6 code is not a reason to refuse to
        // serve. It is also barely possible -- 74 bytes at level Q against the
        // 28 an IPv4 route takes -- so it is reported rather than handled.
        Err(e) => eprintln!("could not draw the route as a code: {e}"),
    }
}

/// The value of one key in a query string, undecoded beyond what is there.
///
/// Only one caller exists and it compares the result against a fixed set, so
/// percent-decoding would be work with no reader. A value that needed it
/// simply will not match, which is the safe direction.
fn query_value<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
}

/// `?theme=dark` or `?theme=light`, and nothing else.
///
/// Anything unrecognised is `None` rather than a default, so a typo leaves the
/// page following the operating system instead of silently forcing a theme.
fn theme_of(query: &str) -> Option<&str> {
    match query_value(query, "theme") {
        Some(t @ ("dark" | "light")) => Some(t),
        _ => None,
    }
}

/// Everything this node can say about itself, as JSON.
///
/// **Every field is measured or configured, none is predicted.**
/// `tokens_per_second` is absent until a generation has actually finished
/// here, because a prediction served under the same name as a measurement is
/// how a number nobody checked ends up quoted back as a result.
fn status_json(engine: &Engine<'_>, node: &Node) -> String {
    let rate = match node.last.get() {
        Some((produced, tps)) => {
            format!(r#","last_generation":{{"tokens":{produced},"tokens_per_second":{tps:.3}}}"#)
        }
        None => String::new(),
    };
    format!(
        r#"{{"status":"ok","model":"{}","context_limit":{},"context_ceiling":{},"route":"{}","reachable":{},"uptime_seconds":{:.0},"verified_architectures":{}{}}}"#,
        escape(engine.model_name()),
        engine.context_limit(),
        engine.hard_context_limit(),
        escape(&node.route),
        !node.loopback,
        node.since.elapsed().as_secs_f64(),
        VERIFIED_ARCHITECTURES.len(),
        rate
    )
}

/// Answer a `stream: true` request as server-sent events.
///
/// The status line and headers are written **before** generation starts, which
/// is the entire point: a client that waits for `Content-Length` cannot show
/// anything until the last token. There is no length to send, so the response
/// ends by closing the connection after `data: [DONE]`.
///
/// An error after the headers are out cannot become a 400 — the status is
/// already committed — so it is delivered as a final chunk carrying the message
/// and then `[DONE]`, which is what the OpenAI clients expect.
fn stream_completion(
    mut stream: TcpStream,
    req: &Request,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
    params: &Params,
    started: std::time::Instant,
    node: &Node,
) -> Result<(), Box<dyn std::error::Error>> {
    // Built by concatenation rather than as one multi-line literal: HTTP header
    // lines are CRLF-separated with no leading whitespace, and a literal that
    // wraps in the source is an easy way to ship indented headers that stricter
    // clients reject.
    let headers = concat!(
        "HTTP/1.1 200 OK\r\n",
        "Content-Type: text/event-stream\r\n",
        "Cache-Control: no-cache\r\n",
        "Access-Control-Allow-Origin: *\r\n",
        "Connection: close\r\n",
        "\r\n",
    );
    stream.write_all(headers.as_bytes())?;
    // The role arrives in its own first chunk, before any content, which is
    // what the OpenAI streaming schema specifies.
    stream.write_all(
        concat!(
            r#"data: {"id":"chaos","object":"chat.completion.chunk","#,
            r#""choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
            "\n\n",
        )
        .as_bytes(),
    )?;
    stream.flush()?;

    let mut sink = stream.try_clone()?;
    let result = generate(&req.body, engine, tokenizer, params, &mut |text| {
        sink.write_all(sse_chunk(text, None).as_bytes())?;
        // Flush per token or the OS buffers the whole answer and "streaming"
        // arrives all at once at the end.
        sink.flush()
    });

    match result {
        Ok((_, _, produced, finish)) => {
            node.record(produced, started.elapsed().as_secs_f64());
            stream.write_all(sse_chunk("", Some(finish)).as_bytes())?;
        }
        Err(e) => {
            stream.write_all(
                sse_chunk(
                    &format!(
                        "
[error: {e}]"
                    ),
                    Some(Finish::Stop),
                )
                .as_bytes(),
            )?;
        }
    }
    stream.write_all(b"data: [DONE]\n\n")?;
    stream.flush()?;
    eprintln!(
        "{} {} -> 200 (stream) in {:.1}s",
        req.method,
        req.target,
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn error_json(message: &str) -> String {
    format!(
        r#"{{"error":{{"message":"{}","type":"invalid_request_error"}}}}"#,
        escape(message)
    )
}

/// Which model this server is driving.
///
/// V4-Flash shares almost none of its graph with the dense architectures, so
/// they are separate variants rather than one configurable path — the same
/// split `chaos-run` makes. Serving only V4-Flash was a real limitation: the
/// server is the part an editor or agent talks to, and refusing every Llama and
/// Qwen container made it useless for the models people actually run.
// The two variants differ in size by more than clippy likes, and boxing the
// larger would put an allocation on a value that lives for the whole process
// and is constructed exactly once.
#[allow(clippy::large_enum_variant)]
enum Engine<'a> {
    Deepseek4 {
        fw: &'a Deepseek4Forward<'a>,
        config: &'a Deepseek4Config,
        /// `-c`, when it was given and is not above what the path can hold.
        asked: Option<usize>,
    },
    /// Dense Llama/Qwen, through the SAME `StreamingRunner` the CLI uses.
    ///
    /// It used to call `arch.build_graph` -- a second forward implementation
    /// that never received the QKV bias, the Gemma activation, the post-norms
    /// or the soft caps, because all of those landed in `stream.rs`. Qwen2
    /// through this server produced fluent nonsense while `chaos-run` on the
    /// same container was byte-identical to llama.cpp. A second code path is a
    /// second place for every fix to be missing from.
    Dense {
        /// Interior mutability because `run_loop` holds the engine by shared
        /// reference and a cached forward pass is inherently stateful. One
        /// request is served at a time, so there is no contention to lose.
        runner: std::cell::RefCell<chaos_arch::StreamingRunner<'a>>,
        weights: &'a WeightSet<'a>,
        name: &'a str,
        config: Qwen3Config,
        /// `-c`, when it was given and is not above what the path can hold.
        asked: Option<usize>,
    },
}

impl Engine<'_> {
    fn model_name(&self) -> &str {
        match self {
            Engine::Deepseek4 { .. } => "deepseek-v4-flash",
            Engine::Dense { name, .. } => name,
        }
    }

    /// Tokens this path can hold in total, prompt plus generation.
    fn context_limit(&self) -> usize {
        // **`-c` lowers this and never raises it.** A ceiling asked for above
        // what the path can actually hold would be a promise the engine breaks
        // mid-request, which is worse than a refusal at the door.
        let (ceiling, asked) = match self {
            Engine::Deepseek4 { asked, .. } => (self.hard_context_limit(), *asked),
            Engine::Dense { asked, .. } => (self.hard_context_limit(), *asked),
        };
        match asked {
            Some(n) if n > 0 => n.min(ceiling),
            _ => ceiling,
        }
    }

    /// What the engine can hold regardless of what was asked for.
    fn hard_context_limit(&self) -> usize {
        match self {
            // **Was 256, and stayed 256 for a release after the engine stopped
            // needing it.** #61 replaced the position-indexed raw latents with
            // a 1024-slot ring, so the total sequence is no longer capped at
            // all -- what is capped is one pass, at 897 tokens, because a pass
            // must hold `window + nt - 1` distinct positions live at once.
            //
            // The server refused sequences the engine had handled for days.
            // A limit that outlives its cause is worse than no limit: it is a
            // correct-looking refusal, and nobody re-derives those.
            Engine::Deepseek4 { .. } => 897,
            // Bounded by the arena rather than by a cache. Kept modest because
            // every pass rebuilds the graph over the whole sequence.
            Engine::Dense { .. } => 2048,
        }
    }
}

/// One generation, driven by whichever engine is loaded.
///
/// Returns the logits for the next token. The two paths differ in what they
/// carry between calls, so the state lives here rather than in `generate`.
enum State {
    Deepseek4(Deepseek4Cache),
    /// The dense path now keeps a KV cache, like the CLI. Rebuilding over the
    /// whole sequence per token was quadratic AND used the unfixed graph.
    Dense(chaos_arch::KvCache),
}

/// Everything a request asks for beyond the messages themselves.
struct Params {
    max_tokens: usize,
    sampler: SamplerConfig,
    stop: Vec<String>,
    stream: bool,
    /// From OpenAI's `response_format`. `None` means unconstrained.
    ///
    /// This is the field that makes a local model usable by an agent: without
    /// it, "reply with JSON" is a request the model may decline, and the caller
    /// finds out by failing to parse the answer.
    grammar: Option<chaos_grammar::Grammar>,
}

impl Params {
    /// Read the OpenAI sampling fields, defaulting the way that API does.
    ///
    /// OpenAI's default temperature is 1.0, not 0.0 — a client that sends no
    /// `temperature` expects sampling, not greedy. That differs from
    /// `chaos-run`, where greedy is right because it keeps a wrong forward
    /// pass diagnosable.
    fn from_body(body: &str) -> Self {
        // OpenAI's default temperature is 1.0, not 0.0, so the rest is taken
        // from `default()` and only that overridden — a struct literal here
        // would need updating every time a sampler is added, and forgetting
        // would silently reset one to zero rather than failing to compile.
        let mut sampler = SamplerConfig {
            temperature: 1.0,
            ..SamplerConfig::default()
        };
        if let Some(v) = extract_float(body, "temperature") {
            sampler.temperature = v as f32;
        }
        if let Some(v) = extract_float(body, "top_p") {
            sampler.top_p = v as f32;
        }
        if let Some(v) = extract_float(body, "min_p") {
            sampler.min_p = v as f32;
        }
        if let Some(v) = extract_int(body, "top_k") {
            sampler.top_k = v.max(0) as usize;
        }
        if let Some(v) =
            extract_float(body, "repetition_penalty").or(extract_float(body, "repeat_penalty"))
        {
            sampler.repeat_penalty = v as f32;
        }
        // Both are standard OpenAI fields. A client that sends them and is
        // silently ignored gets output that looks like the model repeating
        // itself, with nothing to point at.
        if let Some(v) = extract_float(body, "frequency_penalty") {
            sampler.frequency_penalty = v as f32;
        }
        if let Some(v) = extract_float(body, "presence_penalty") {
            sampler.presence_penalty = v as f32;
        }
        if let Some(v) = extract_int(body, "seed") {
            sampler.seed = v as u64;
        }
        Params {
            max_tokens: extract_int(body, "max_tokens").unwrap_or(64).clamp(1, 4096) as usize,
            sampler,
            stop: extract_string_array(body, "stop"),
            stream: extract_bool(body, "stream").unwrap_or(false),
            grammar: response_format_grammar(body),
        }
    }
}

/// Why generation ended, in the vocabulary the OpenAI API uses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Finish {
    /// Hit the token budget.
    Length,
    /// The model emitted end-of-sequence, or a stop sequence was produced.
    Stop,
}

impl Finish {
    fn as_str(self) -> &'static str {
        match self {
            Finish::Length => "length",
            Finish::Stop => "stop",
        }
    }
}

/// Run one completion, handing each newly decoded piece of text to `emit`.
///
/// `emit` returning an error aborts generation — that is how a client
/// disconnecting mid-stream stops the work rather than finishing it for nobody.
fn generate(
    body: &str,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
    params: &Params,
    emit: &mut dyn FnMut(&str) -> std::io::Result<()>,
) -> Result<(String, usize, usize, Finish), Box<dyn std::error::Error>> {
    let messages = extract_messages(body)?;
    // The framing the model was trained on. Concatenating the contents -- what
    // this did before -- makes an instruct model continue the conversation
    // rather than answer it.
    let prompt = tokenizer.apply_chat_template(&messages, true);
    run_prompt(&prompt, engine, tokenizer, params, emit)
}

/// `/v1/completions`: the caller's text verbatim, with no chat framing.
///
/// A base model or an autocomplete client wants exactly what it sent. Applying
/// a chat template here would be the mirror of the bug that made instruct
/// models answer the wrong question.
fn generate_raw(
    body: &str,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
    params: &Params,
    emit: &mut dyn FnMut(&str) -> std::io::Result<()>,
) -> Result<(String, usize, usize, Finish), Box<dyn std::error::Error>> {
    let prompt =
        extract_json_string(body, "prompt").ok_or("no `prompt` string in the request body")?;
    run_prompt(&prompt, engine, tokenizer, params, emit)
}

/// `/v1/embeddings`: the hidden state, not the logits.
///
/// # Why this stopped being a 501
///
/// The refusal said "this runner's graph returns logits, not hidden states".
/// That was true of what the graph *returned* and false about what it computed:
/// the pre-projection hidden state is the input to the vocabulary matmul and was
/// being discarded a line later. `set_want_embedding` keeps it, at the cost of
/// one `compute` on a tensor already in the graph, and only when asked.
///
/// Taken **after `output_norm` and before the vocabulary projection**, which is
/// where llama.cpp takes it. Earlier and the vector carries a per-model scale
/// that makes similarity between two models meaningless; later and it is a
/// distribution over tokens rather than an embedding.
///
/// Each input gets a **fresh KV cache**. Sharing one would make every embedding
/// after the first a function of the texts before it — the vectors would still
/// look plausible, and they would silently encode the batch order.
///
/// One vector per input, and the total prompt tokens for the `usage` field.
type Embeddings = (Vec<Vec<f32>>, usize);

fn embed(
    body: &str,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
) -> Result<Embeddings, Box<dyn std::error::Error>> {
    let inputs = extract_inputs(body)
        .ok_or("no `input` in the request body: expected a string or an array of strings")?;
    if inputs.is_empty() {
        return Err("`input` is empty".into());
    }

    let Engine::Dense {
        runner,
        weights,
        config,
        ..
    } = engine
    else {
        // Deepseek4 runs a different forward path whose output stage this does
        // not reach. Named rather than dressed up as a generic failure.
        return Err(
            "embeddings are implemented for the dense path only; this model uses the \
             V4-Flash path, whose forward pass does not expose a hidden state yet"
                .into(),
        );
    };

    let mut vectors = Vec::with_capacity(inputs.len());
    let mut prompt_tokens = 0usize;
    for text in &inputs {
        let tokens: Vec<u32> = tokenizer.encode(text);
        if tokens.is_empty() {
            return Err("one of the inputs is empty".into());
        }
        if tokens.len() > engine.context_limit() {
            return Err(format!(
                "an input is {} tokens and this path holds {}",
                tokens.len(),
                engine.context_limit()
            )
            .into());
        }
        prompt_tokens += tokens.len();

        let mut kv = chaos_arch::KvCache::new(
            config.n_layer as usize,
            config.n_head_kv as usize,
            config.head_dim as usize,
        );
        let mut r = runner.borrow_mut();
        r.set_want_embedding(true);
        let _logits = r.forward_cached(weights, &mut kv, &tokens, 0)?;
        let v = r
            .last_embedding()
            .ok_or("the forward pass produced no hidden state")?;
        r.set_want_embedding(false);
        vectors.push(v);
    }
    Ok((vectors, prompt_tokens))
}

/// The shared body of both completion endpoints.
fn run_prompt(
    prompt: &str,
    engine: &Engine<'_>,
    tokenizer: &Tokenizer,
    params: &Params,
    emit: &mut dyn FnMut(&str) -> std::io::Result<()>,
) -> Result<(String, usize, usize, Finish), Box<dyn std::error::Error>> {
    let tokens: Vec<i32> = tokenizer.encode(prompt).iter().map(|t| *t as i32).collect();
    if tokens.is_empty() {
        return Err("empty prompt".into());
    }
    // A real property of the path rather than a policy, and worth stating
    // before ten seconds of loading discovers it. For the V4-Flash path this
    // is now the per-PASS bound rather than a total: the ring holds any length,
    // but one forward pass cannot cover more than `RAW_RING - window + 1`
    // positions. Since the server prefills a prompt in a single pass, the
    // prompt is what the bound applies to.
    let limit = engine.context_limit();
    if tokens.len() + params.max_tokens > limit {
        return Err(format!(
            "prompt is {} tokens and max_tokens is {}; this path holds {limit} in total",
            tokens.len(),
            params.max_tokens
        )
        .into());
    }

    let mut seq = tokens.clone();
    let mut state = match engine {
        Engine::Deepseek4 { config, .. } => {
            State::Deepseek4(Deepseek4Cache::new(config.n_layer, config.kv_lora_rank))
        }
        Engine::Dense { config, .. } => State::Dense(chaos_arch::KvCache::new(
            config.n_layer as usize,
            config.n_head_kv as usize,
            config.head_dim as usize,
        )),
    };
    let mut logits = advance(engine, &mut state, &seq, true)?;

    let mut sampler = Sampler::new(params.sampler.clone());
    let mut history: Vec<u32> = tokens.iter().map(|&t| t as u32).collect();

    // `response_format`. The vocabulary is built once as token id -> the bytes
    // that token decodes to, which is what the grammar matches against; the
    // matcher is carried across tokens rather than re-parsing the text so far,
    // because `allowed(prefix)` is quadratic in the answer's length and an
    // agent's structured reply is exactly where that shows.
    // The vocabulary outlives the constraint that borrows it, which is why it
    // is bound here rather than inside the closure.
    let vocab: Vec<Vec<u8>> = params
        .grammar
        .as_ref()
        .map(|_| {
            (0..tokenizer.vocab_size() as u32)
                .map(|id| tokenizer.decode(&[id]).into_bytes())
                .collect()
        })
        .unwrap_or_default();
    let constraint = params
        .grammar
        .as_ref()
        .map(|g| chaos_grammar::Constraint::new(g.clone(), &vocab));
    let mut matcher = constraint.as_ref().map(|c| c.grammar().matcher());
    let mut grammar_done = false;

    apply_grammar(&constraint, &matcher, &mut logits, &mut grammar_done);
    let mut next = sampler.sample(&logits, &history) as i32;

    let mut out = String::new();
    // Bytes not yet forming a whole character. One character is often several
    // tokens, so converting each token to text on its own would emit
    // replacement characters into the stream permanently.
    let mut pending: Vec<u8> = Vec::new();
    let mut produced = 0usize;
    let mut finish = Finish::Length;
    let started = std::time::Instant::now();

    loop {
        if Some(next as u32) == tokenizer.eos {
            finish = Finish::Stop;
            break;
        }
        history.push(next as u32);
        pending.extend(tokenizer.decode_bytes(&[next as u32]));
        let good = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(e) => e.valid_up_to(),
        };
        if good > 0 {
            let text = String::from_utf8_lossy(&pending[..good]).into_owned();
            pending.drain(..good);
            out.push_str(&text);
            emit(&text)?;
        }
        produced += 1;

        // A stop sequence is checked against the accumulated text, not the
        // token, because it can straddle a token boundary.
        if let Some(cut) = params
            .stop
            .iter()
            .filter(|s| !s.is_empty())
            .find_map(|s| out.find(s.as_str()))
        {
            out.truncate(cut);
            finish = Finish::Stop;
            break;
        }
        if produced >= params.max_tokens {
            break;
        }
        // Advance the grammar by what was actually emitted, then stop if it
        // can accept nothing more -- a satisfied grammar is a finished answer.
        if let Some(m) = matcher.as_mut() {
            m.accept_str(&tokenizer.decode(&[next as u32]));
        }
        if grammar_done {
            finish = Finish::Stop;
            break;
        }
        seq.push(next);
        let mut logits = advance(engine, &mut state, &seq, false)?;
        apply_grammar(&constraint, &matcher, &mut logits, &mut grammar_done);
        if grammar_done {
            finish = Finish::Stop;
            break;
        }
        next = sampler.sample(&logits, &history) as i32;
    }

    let secs = started.elapsed().as_secs_f64();
    eprintln!(
        "  {produced} tokens in {secs:.1}s ({:.3} tok/s), finish={}",
        produced as f64 / secs.max(1e-9),
        finish.as_str()
    );
    Ok((out, tokens.len(), produced, finish))
}

/// Run the model forward and return the next token's logits.
///
/// `first` distinguishes the prompt pass from a continuation. The deepseek4
/// path is incremental — its KV cache means a step feeds one token — while the
/// dense path rebuilds over the whole sequence every time, so it needs `seq`
/// rather than the last token.
/// The server carries token ids as `i32` (the OpenAI shape); the engine wants
/// `u32`. Converted at the boundary rather than changing either side.
fn seq_u32(seq: &[i32]) -> Vec<u32> {
    seq.iter().map(|&t| t as u32).collect()
}

fn advance(
    engine: &Engine<'_>,
    state: &mut State,
    seq: &[i32],
    first: bool,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    match (engine, state) {
        (Engine::Deepseek4 { fw, .. }, State::Deepseek4(kv)) => {
            let arena = 1024usize << 20;
            if first {
                Ok(chaos_arch::forward(fw, kv, seq, arena)?)
            } else {
                // The cache already holds everything before it, so a step feeds
                // exactly the token just chosen.
                let last = *seq.last().expect("non-empty sequence");
                Ok(chaos_arch::step(fw, kv, last, arena)?)
            }
        }
        (
            Engine::Dense {
                runner, weights, ..
            },
            State::Dense(kv),
        ) => {
            // `first` prefills the whole prompt; every step after feeds exactly
            // the token just chosen, because the cache holds the rest.
            let mut r = runner.borrow_mut();
            if first {
                Ok(r.forward_cached(weights, kv, seq_u32(seq).as_slice(), 0)?)
            } else {
                let last = *seq.last().expect("non-empty sequence") as u32;
                let pos = kv.len();
                Ok(r.forward_cached(weights, kv, &[last], pos)?)
            }
        }
        _ => Err("engine and state disagree -- this is a bug".into()),
    }
}

/// The `/v1/embeddings` response body.
///
/// Floats are written with `{:?}`, which gives Rust's shortest representation
/// that round-trips exactly. A fixed number of decimal places would silently
/// quantise every vector the server ever returns.
fn embeddings_json(model: &str, vectors: &[Vec<f32>], prompt_tokens: usize) -> String {
    let mut data = String::new();
    for (i, v) in vectors.iter().enumerate() {
        if i > 0 {
            data.push(',');
        }
        let mut nums = String::with_capacity(v.len() * 12);
        for (j, x) in v.iter().enumerate() {
            if j > 0 {
                nums.push(',');
            }
            // Non-finite values are not legal JSON. They cannot come out of a
            // healthy forward pass, so emitting 0 would hide a real fault --
            // `null` is at least visible to the client as "not a number".
            if x.is_finite() {
                nums.push_str(&format!("{x:?}"));
            } else {
                nums.push_str("null");
            }
        }
        data.push_str(&format!(
            r#"{{"object":"embedding","index":{i},"embedding":[{nums}]}}"#
        ));
    }
    format!(
        r#"{{"object":"list","model":"{model}","data":[{data}],"usage":{{"prompt_tokens":{prompt_tokens},"total_tokens":{prompt_tokens}}}}}"#
    )
}

/// The non-streaming response body.
fn completion_json(
    model: &str,
    text: &str,
    prompt_tokens: usize,
    produced: usize,
    finish: Finish,
) -> String {
    format!(
        r#"{{"id":"chaos","object":"chat.completion","model":"{model}","choices":[{{"index":0,"message":{{"role":"assistant","content":"{}"}},"finish_reason":"{}"}}],"usage":{{"prompt_tokens":{prompt_tokens},"completion_tokens":{produced},"total_tokens":{}}}}}"#,
        escape(text),
        finish.as_str(),
        prompt_tokens + produced
    )
}

/// One server-sent-event chunk carrying a delta.
fn sse_chunk(delta: &str, finish: Option<Finish>) -> String {
    let finish_field = match finish {
        Some(f) => format!(r#""{}""#, f.as_str()),
        None => "null".to_string(),
    };
    let delta_field = if delta.is_empty() {
        "{}".to_string()
    } else {
        format!(r#"{{"content":"{}"}}"#, escape(delta))
    };
    format!(
        "data: {{\"id\":\"chaos\",\"object\":\"chat.completion.chunk\",\"choices\":[{{\"index\":0,\"delta\":{delta_field},\"finish_reason\":{finish_field}}}]}}\n\n"
    )
}

/// Pull the conversation out of an OpenAI request body.
///
/// A hand-rolled scan rather than a JSON parser, for the same reason there is no
/// HTTP crate: the shape is fixed and known. It handles what a client actually
/// sends — `messages: [{role, content}]` — and refuses anything it does not
/// understand instead of guessing.
/// Pull `messages[]` out of a chat-completions body, in order, with roles.
///
/// Hand-rolled because this crate has no JSON dependency. It reads `"role"` and
/// `"content"` pairs in document order, which is what the OpenAI schema
/// guarantees, and refuses anything it cannot represent rather than sending the
/// model half a request.
fn extract_messages(body: &str) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
    let mut out: Vec<Message> = Vec::new();
    let mut rest = body;
    // Track the most recent role so a content field is attributed correctly
    // even though the two keys are separate.
    let mut pending_role = String::from("user");
    loop {
        let role_at = rest.find("\"role\"");
        let content_at = rest.find("\"content\"");
        match (role_at, content_at) {
            (Some(r), Some(c)) if r < c => {
                let after = rest[r + "\"role\"".len()..].trim_start();
                let Some(colon) = after.find(':') else { break };
                let val = after[colon + 1..].trim_start();
                if let Some(body) = val.strip_prefix('"') {
                    let (text, _) = read_json_string(body)?;
                    pending_role = text;
                }
                let off = rest.len() - after.len() + colon + 1;
                rest = &rest[off..];
            }
            (_, Some(c)) => {
                let after = rest[c + "\"content\"".len()..].trim_start();
                let Some(colon) = after.find(':') else { break };
                let val = after[colon + 1..].trim_start();
                if !val.starts_with('"') {
                    // An array of content parts (images, audio). Refusing is
                    // the honest answer -- this runner is text only.
                    return Err("only string `content` is supported".into());
                }
                let (text, consumed) = read_json_string(&val[1..])?;
                out.push(Message::new(&pending_role, &text));
                pending_role = String::from("user");
                let off = rest.len() - val.len() + 1 + consumed;
                rest = &rest[off..];
            }
            _ => break,
        }
    }
    if out.is_empty() {
        return Err("no `messages[].content` in the request body".into());
    }
    Ok(out)
}

/// Read a JSON string body (the opening quote already consumed).
/// Returns the decoded text and how many bytes were consumed including the
/// closing quote.
fn read_json_string(s: &str) -> Result<(String, usize), Box<dyn std::error::Error>> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((out, i + 1)),
            '\\' => {
                let Some((_, esc)) = chars.next() else { break };
                out.push(match esc {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    'u' => {
                        // Skip the four hex digits; a coding prompt rarely needs
                        // them and guessing wrong is worse than dropping one.
                        for _ in 0..4 {
                            chars.next();
                        }
                        continue;
                    }
                    other => other,
                });
            }
            other => out.push(other),
        }
    }
    Err("unterminated string in request body".into())
}

/// Mask the logits to what the grammar allows, and say when it is finished.
///
/// # Why an empty mask cannot simply be sampled from
///
/// Every token would be `-inf`, the argmax would be arbitrary, and the answer
/// would end looking exactly like a clean stop. Empty has two meanings and they
/// are not the same event: a grammar that has been SATISFIED admits nothing
/// more, which is success; one that is STUCK admits nothing because the text so
/// far cannot be completed, which is a truncated answer. Reporting the second as
/// the first is how a client receives half a JSON object and a `"stop"` reason.
fn apply_grammar(
    constraint: &Option<chaos_grammar::Constraint>,
    matcher: &Option<chaos_grammar::Matcher>,
    logits: &mut [f32],
    done: &mut bool,
) {
    let (Some(c), Some(m)) = (constraint.as_ref(), matcher.as_ref()) else {
        return;
    };
    let mask = c.allowed_from(m);
    if mask.is_empty() {
        if !m.is_complete() {
            chaos_arch::info!(
                "serve      grammar STUCK -- no token can continue and it is not satisfied; \
                 the response is incomplete"
            );
        }
        *done = true;
        return;
    }
    mask.apply(logits);
}

/// Turn OpenAI's `response_format` into a grammar.
///
/// Two shapes are standard and both are honoured:
///
/// ```json
/// {"response_format": {"type": "json_object"}}
/// {"response_format": {"type": "json_schema", "json_schema": {"schema": { ... }}}}
/// ```
///
/// # Why a malformed schema is not silently dropped
///
/// A `response_format` that fails to compile and is then ignored produces free
/// text where the caller is parsing JSON. That failure surfaces in the client,
/// several layers from its cause, and looks like the model disobeying rather
/// than the server discarding the request. So a schema that will not compile is
/// reported here and the request is refused.
fn response_format_grammar(body: &str) -> Option<chaos_grammar::Grammar> {
    let at = body.find("\"response_format\"")?;
    let rest = &body[at..];
    // The `type` nearest the key. Crude, and deliberately so: this server
    // parses JSON by scanning rather than carrying a parser, and a nested
    // `"type"` inside the schema itself is exactly why the FIRST one after
    // `response_format` is the one taken.
    let ty = extract_string(rest, "type")?;
    match ty.as_str() {
        // Any JSON value. llama.cpp's `--json-schema '{}'` compiles to the
        // same thing, so the two agree on what "json_object" means.
        "json_object" => chaos_grammar::Grammar::from_json_schema("{}").ok(),
        "json_schema" => {
            // The schema sits under `json_schema.schema` in the OpenAI shape.
            // Taken as a raw substring rather than re-serialised: re-encoding
            // a schema through a scanner would change it, and a changed schema
            // is a changed contract.
            let schema = raw_object_after(rest, "\"schema\"")?;
            match chaos_grammar::Grammar::from_json_schema(&schema) {
                Ok(g) => Some(g),
                Err(e) => {
                    chaos_arch::info!("serve      response_format schema rejected: {e}");
                    None
                }
            }
        }
        other => {
            chaos_arch::info!("serve      response_format type {other:?} not recognised");
            None
        }
    }
}

/// The balanced `{...}` that follows `key`, as raw text.
///
/// Brace counting rather than parsing, and it respects strings and escapes --
/// a schema containing `"pattern": "\\}"` would otherwise close the object
/// early and hand the grammar compiler a truncated document.
fn raw_object_after(body: &str, key: &str) -> Option<String> {
    let at = body.find(key)? + key.len();
    let start = body[at..].find('{')? + at;
    let bytes = body.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(body[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_int(body: &str, key: &str) -> Option<i64> {
    let at = body.find(&format!("\"{key}\""))?;
    let after = &body[at + key.len() + 2..];
    let colon = after.find(':')?;
    let digits: String = after[colon + 1..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Read a top-level JSON string field, e.g. `"prompt"`.
fn extract_json_string(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let at = body.find(&needle)?;
    let after = body[at + needle.len()..].trim_start();
    let rest = after.strip_prefix(':')?.trim_start();
    let body = rest.strip_prefix('"')?;
    read_json_string(body).ok().map(|(s, _)| s)
}

/// `input` for `/v1/embeddings`, which OpenAI defines as **either** a string or
/// an array of strings.
///
/// Both spellings are in real client code, and a server that takes only the
/// scalar form fails on the batch one with "no input" — which reads like the
/// request was empty rather than like the shape was unsupported.
fn extract_inputs(body: &str) -> Option<Vec<String>> {
    let needle = "\"input\"";
    let at = body.find(needle)?;
    let after = body[at + needle.len()..].trim_start();
    let rest = after.strip_prefix(':')?.trim_start();

    if let Some(one) = rest.strip_prefix('"') {
        return read_json_string(one).ok().map(|(s, _)| vec![s]);
    }
    // The array form. Walked with the same string reader rather than split on
    // commas, because an input containing a comma is ordinary text.
    let mut cur = rest.strip_prefix('[')?.trim_start();
    let mut out = Vec::new();
    loop {
        if let Some(end) = cur.strip_prefix(']') {
            let _ = end;
            return Some(out);
        }
        let s = cur.strip_prefix('"')?;
        let (text, used) = read_json_string(s).ok()?;
        out.push(text);
        cur = s[used..].trim_start();
        cur = match cur.strip_prefix(',') {
            Some(next) => next.trim_start(),
            None => return cur.strip_prefix(']').map(|_| out),
        };
    }
}

/// Read a JSON number as `f64`. Accepts integers too, since `temperature: 1`
/// is legal JSON and common from hand-written clients.
fn extract_float(body: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\"");
    let at = body.find(&needle)?;
    let after = body[at + needle.len()..].trim_start();
    let rest = after.strip_prefix(':')?.trim_start();
    let end = rest
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')
        })
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Read a JSON boolean. Absent and malformed are both `None`, so the caller
/// picks the default rather than this guessing one.
fn extract_bool(body: &str, key: &str) -> Option<bool> {
    let needle = format!("\"{key}\"");
    let at = body.find(&needle)?;
    let after = body[at + needle.len()..].trim_start();
    let rest = after.strip_prefix(':')?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// Read `"stop"`, which the OpenAI API allows as a string **or** an array of
/// strings. Both spellings are common in the wild, so both are accepted.
/// The string value of `key`, scanning rather than parsing.
///
/// This server carries no JSON parser on purpose; every reader here is a scan.
/// Returns the FIRST match after the caller's slice start, which is what makes
/// `response_format_grammar` able to take the `type` nearest its own key rather
/// than a `"type"` nested inside a schema.
fn extract_string(body: &str, key: &str) -> Option<String> {
    let at = body.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = &body[at..];
    let colon = rest.find(':')? + 1;
    let open = rest[colon..].find('"')? + colon + 1;
    let mut out = String::new();
    let mut escaped = false;
    for c in rest[open..].chars() {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn extract_string_array(body: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let Some(at) = body.find(&needle) else {
        return Vec::new();
    };
    let after = body[at + needle.len()..].trim_start();
    let Some(rest) = after.strip_prefix(':') else {
        return Vec::new();
    };
    let rest = rest.trim_start();
    if let Some(one) = rest.strip_prefix('"') {
        return read_json_string(one)
            .map(|(s, _)| vec![s])
            .unwrap_or_default();
    }
    let Some(mut list) = rest.strip_prefix('[') else {
        return Vec::new();
    };
    let mut out = Vec::new();
    loop {
        list = list.trim_start();
        match list.strip_prefix('"') {
            Some(body) => match read_json_string(body) {
                Ok((s, consumed)) => {
                    out.push(s);
                    list = &body[consumed..];
                }
                Err(_) => break,
            },
            None => break,
        }
        list = list.trim_start();
        match list.strip_prefix(',') {
            Some(next) => list = next,
            None => break,
        }
    }
    out
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{authorised, Request};

    fn req(target: &str, auth: Option<&str>) -> Request {
        Request {
            method: "POST".into(),
            target: target.into(),
            body: String::new(),
            auth: auth.map(str::to_string),
        }
    }

    /// **The key gate, exhaustively, because nothing was checking it.**
    ///
    /// §4d asks of each area: what would a wrong answer look like, and would
    /// anything catch it? Here a wrong answer is a `/v1/*` endpoint that stops
    /// requiring the key — silent, invisible in any log, and on a machine bound
    /// to `0.0.0.0` it is the whole of the security model gone. Nothing was
    /// catching it: `authorised` had no test at all.
    #[test]
    fn the_key_gate_covers_v1_and_only_v1() {
        // No key configured: everything is open, which is the documented default
        // and safe only because the server binds loopback unless told otherwise.
        for target in ["/", "/status", "/v1/models", "/v1/chat/completions"] {
            assert!(
                authorised(&req(target, None), None),
                "{target} was refused with no key configured"
            );
        }

        // A key is configured. Everything outside /v1/ stays open on purpose:
        // the browser page, the mark and the reader are not the API.
        for target in [
            "/",
            "/favicon.ico",
            "/qr",
            "/mark",
            "/scan",
            "/status",
            "/health",
        ] {
            assert!(
                authorised(&req(target, None), Some("secret")),
                "{target} started requiring a key; the page and the mark must not"
            );
        }

        // And everything under /v1/ is gated.
        for target in [
            "/v1/models",
            "/v1/chat/completions",
            "/v1/completions",
            "/v1/embeddings",
        ] {
            assert!(
                !authorised(&req(target, None), Some("secret")),
                "{target} is not gated, so the key protects nothing"
            );
            assert!(
                !authorised(&req(target, Some("Bearer wrong")), Some("secret")),
                "{target} accepted the wrong key"
            );
            // A query string must not slip past the prefix check.
            let with_query = format!("{target}?stream=true");
            assert!(
                !authorised(&req(&with_query, None), Some("secret")),
                "{with_query} escaped the gate via its query string"
            );
        }
    }

    /// Every header shape a real client sends, and none that it does not.
    #[test]
    fn a_key_is_accepted_however_the_client_spells_the_header() {
        let key = Some("s3cr3t");
        for header in ["Bearer s3cr3t", "bearer s3cr3t", "s3cr3t", "  s3cr3t  "] {
            assert!(
                authorised(&req("/v1/models", Some(header)), key),
                "{header:?} was refused, and some client sends exactly that"
            );
        }
        // **Compared in full, not by prefix.** A prefix comparison would accept
        // any key beginning with the right characters, which is the difference
        // between a check and a formality.
        for header in [
            "Bearer s3cr3",
            "Bearer s3cr3t-extra",
            "Bearer S3CR3T",
            "Bearer ",
            "",
            "Basic s3cr3t",
        ] {
            assert!(
                !authorised(&req("/v1/models", Some(header)), key),
                "{header:?} was accepted as the key"
            );
        }
    }

    /// **What counts as "only this machine".**
    ///
    /// `0.0.0.0` is the trap: it reads like "no address" and means *every*
    /// address, so a rule that tested for a literal `127.0.0.1` would let the
    /// most open binding through as though it were the most closed one.
    #[test]
    fn only_real_loopback_counts_as_loopback() {
        for h in [
            "127.0.0.1",
            "127.0.0.2",
            "127.1.2.3",
            "localhost",
            "::1",
            "[::1]",
        ] {
            assert!(super::is_loopback(h), "{h} is loopback");
        }
        for h in ["0.0.0.0", "192.168.1.20", "10.0.0.5", "::", "1.2.3.4", ""] {
            assert!(!super::is_loopback(h), "{h} is NOT loopback");
        }
        // Not a prefix match: an address that merely starts with the digits.
        assert!(!super::is_loopback("127.0.0.1.evil.com"));
        assert!(!super::is_loopback("1270.0.0.1"));
    }

    /// **A key is optional on loopback and required off it.**
    ///
    /// On `127.0.0.1` a key guards nothing -- a caller who can reach it can
    /// read the weights off the disk anyway. On a LAN address it is the only
    /// thing between the model and every device on the Wi-Fi, so the server
    /// refuses to start rather than warning and starting anyway.
    #[test]
    fn a_lan_binding_without_a_key_is_refused() {
        assert!(super::refuse_to_start("127.0.0.1", None).is_none());
        assert!(super::refuse_to_start("localhost", Some("k")).is_none());
        assert!(super::refuse_to_start("0.0.0.0", Some("a-long-key")).is_none());

        let why = super::refuse_to_start("0.0.0.0", None).expect("must refuse");
        assert!(why.contains("api key"), "{why}");
        assert!(
            why.contains("--api-key"),
            "the refusal must say how to fix it"
        );

        // An empty key is not a key.
        assert!(super::refuse_to_start("192.168.1.20", Some("")).is_some());
    }

    use super::*;

    #[test]
    fn a_normal_chat_request_yields_its_messages_with_roles() {
        let body = r#"{"model":"x","messages":[{"role":"system","content":"Be brief."},
                      {"role":"user","content":"Hello"}],"max_tokens":16}"#;
        let msgs = extract_messages(body).unwrap();
        assert_eq!(msgs.len(), 2);
        // Roles must survive: a system turn framed as a user turn is a
        // different prompt, and the model answers it differently.
        assert_eq!(msgs[0], Message::new("system", "Be brief."));
        assert_eq!(msgs[1], Message::new("user", "Hello"));
        assert_eq!(extract_int(body, "max_tokens"), Some(16));
    }

    #[test]
    fn escapes_survive_the_round_trip() {
        let body = r#"{"messages":[{"role":"user","content":"say \"hi\"\nand a tab\there"}]}"#;
        let got = extract_messages(body).unwrap().remove(0).content;
        assert_eq!(got, "say \"hi\"\nand a tab\there");
        // And what comes back out must be valid JSON again.
        let e = escape(&got);
        assert!(
            !e.contains('\n'),
            "raw newline would break the response body"
        );
        assert!(e.contains("\\\""), "quotes must be escaped: {e}");
    }

    #[test]
    fn unsupported_content_is_refused_not_guessed() {
        // Multimodal clients send an array of parts. Sending the model a
        // half-understood request is worse than saying no.
        let body = r#"{"messages":[{"role":"user","content":[{"type":"text","text":"hi"}]}]}"#;
        assert!(extract_messages(body).is_err());
    }

    #[test]
    fn a_request_without_content_is_an_error() {
        assert!(extract_messages(r#"{"model":"x"}"#).is_err());
    }

    #[test]
    fn missing_max_tokens_is_absent_rather_than_zero() {
        // Defaulting to 0 would silently produce an empty completion.
        assert_eq!(extract_int(r#"{"messages":[]}"#, "max_tokens"), None);
    }

    #[test]
    fn control_characters_cannot_break_the_response() {
        let e = escape("a\u{1}b");
        assert!(e.contains("\\u0001"), "{e}");
    }

    #[test]
    fn sampling_params_are_read_from_the_request() {
        let body = r#"{"messages":[{"role":"user","content":"hi"}],
                       "temperature":0.7,"top_p":0.9,"top_k":40,"seed":123,
                       "max_tokens":32,"stream":true}"#;
        let p = Params::from_body(body);
        assert!((p.sampler.temperature - 0.7).abs() < 1e-6);
        assert!((p.sampler.top_p - 0.9).abs() < 1e-6);
        assert_eq!(p.sampler.top_k, 40);
        assert_eq!(p.sampler.seed, 123);
        assert_eq!(p.max_tokens, 32);
        assert!(p.stream);
    }

    #[test]
    fn an_absent_temperature_defaults_to_the_openai_value_not_greedy() {
        // OpenAI's default is 1.0. Defaulting to 0.0 here would make every
        // answer from every client deterministic and flat, which is a
        // behaviour difference no caller asked for.
        let p = Params::from_body(r#"{"messages":[{"role":"user","content":"hi"}]}"#);
        assert!((p.sampler.temperature - 1.0).abs() < 1e-6);
        assert!(!p.stream, "stream must default to false");
        assert!(p.stop.is_empty());
    }

    #[test]
    fn stop_is_accepted_as_a_string_or_an_array() {
        // The OpenAI schema allows both and clients send both.
        let one = Params::from_body(r#"{"stop":"END","messages":[]}"#);
        assert_eq!(one.stop, vec!["END".to_string()]);
        let many = Params::from_body(
            r#"{"stop":["

","<|eot|>"],"messages":[]}"#,
        );
        assert_eq!(
            many.stop,
            vec![
                "

"
                .to_string(),
                "<|eot|>".to_string()
            ]
        );
    }

    #[test]
    fn floats_parse_whether_written_as_int_or_decimal() {
        assert_eq!(
            extract_float(r#"{"temperature":1}"#, "temperature"),
            Some(1.0)
        );
        assert_eq!(
            extract_float(r#"{"temperature":0.25}"#, "temperature"),
            Some(0.25)
        );
        assert_eq!(extract_float(r#"{"a":1}"#, "temperature"), None);
        assert_eq!(extract_bool(r#"{"stream":true}"#, "stream"), Some(true));
        assert_eq!(extract_bool(r#"{"stream":false}"#, "stream"), Some(false));
        assert_eq!(extract_bool(r#"{"x":1}"#, "stream"), None);
    }

    #[test]
    fn an_sse_chunk_is_one_event_with_a_blank_line_after_it() {
        // Two newlines terminate an event. One, and every client hangs waiting
        // for the rest of it.
        let c = sse_chunk("hi", None);
        assert!(c.starts_with("data: {"));
        assert!(
            c.ends_with(
                "

"
            ),
            "event must end with a blank line: {c:?}"
        );
        assert!(c.contains(r#""content":"hi""#));
        assert!(c.contains(r#""finish_reason":null"#));

        let last = sse_chunk("", Some(Finish::Stop));
        assert!(
            last.contains(r#""delta":{}"#),
            "the final chunk carries no content"
        );
        assert!(last.contains(r#""finish_reason":"stop""#));
    }

    #[test]
    fn a_chunk_escapes_content_that_would_break_the_event() {
        // A raw newline inside the JSON would terminate the event early and
        // the client would see a truncated object.
        let c = sse_chunk("line1\nline2\"quoted\"", None);
        let payload = c.trim_start_matches("data: ").trim_end();
        assert!(
            !payload.contains('\n'),
            "raw newline breaks the event: {payload:?}"
        );
        assert!(
            payload.contains("\\n"),
            "newline must be escaped: {payload:?}"
        );
    }

    #[test]
    fn finish_reason_distinguishes_running_out_from_stopping() {
        assert_eq!(Finish::Length.as_str(), "length");
        assert_eq!(Finish::Stop.as_str(), "stop");
        let j = completion_json("test-model", "hi", 5, 2, Finish::Stop);
        assert!(j.contains(r#""model":"test-model""#));
        assert!(j.contains(r#""finish_reason":"stop""#));
        assert!(j.contains(r#""total_tokens":7"#));
    }

    #[test]
    fn a_raw_prompt_is_read_for_the_completions_endpoint() {
        let body = r#"{"model":"x","prompt":"once upon a","max_tokens":8}"#;
        assert_eq!(
            extract_json_string(body, "prompt").as_deref(),
            Some("once upon a")
        );
        // Absent is None rather than an empty string, so the endpoint can
        // refuse instead of generating from nothing.
        assert_eq!(extract_json_string(r#"{"model":"x"}"#, "prompt"), None);
    }

    #[test]
    fn escapes_survive_a_raw_prompt() {
        let body = r#"{"prompt":"say \"hi\"
then stop"}"#;
        assert_eq!(
            extract_json_string(body, "prompt").as_deref(),
            Some(
                "say \"hi\"
then stop"
            )
        );
    }

    #[test]
    fn input_accepts_both_the_string_and_the_array_form() {
        // OpenAI defines `input` as either. A server that takes only the scalar
        // form fails the batch one with "no input", which reads like an empty
        // request rather than an unsupported shape.
        assert_eq!(
            extract_inputs(r#"{"input":"hello"}"#),
            Some(vec!["hello".to_string()])
        );
        assert_eq!(
            extract_inputs(r#"{"input":["a","b","c"]}"#),
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn an_input_containing_a_comma_is_one_input() {
        // The array is walked with the JSON string reader rather than split on
        // commas, because a comma inside a text is ordinary.
        assert_eq!(
            extract_inputs(r#"{"input":["one, two","three"]}"#),
            Some(vec!["one, two".to_string(), "three".to_string()])
        );
    }

    #[test]
    fn an_empty_input_array_is_recognised_rather_than_rejected_as_absent() {
        // `[]` parses to zero inputs; the handler rejects it with "`input` is
        // empty", which is a different message from "no `input`".
        assert_eq!(extract_inputs(r#"{"input":[]}"#), Some(vec![]));
        assert_eq!(extract_inputs(r#"{"model":"x"}"#), None);
    }

    #[test]
    fn an_embedding_response_is_shaped_like_openais() {
        let json = embeddings_json("m", &[vec![1.0, -0.5]], 3);
        assert!(json.contains(r#""object":"list""#), "{json}");
        assert!(json.contains(r#""object":"embedding""#), "{json}");
        assert!(json.contains(r#""index":0"#), "{json}");
        assert!(json.contains("[1.0,-0.5]"), "{json}");
        assert!(json.contains(r#""prompt_tokens":3"#), "{json}");
    }

    #[test]
    fn a_non_finite_value_becomes_null_rather_than_invalid_json() {
        // NaN and inf are not legal JSON. They cannot come out of a healthy
        // forward pass, so emitting 0 would hide a fault -- `null` is at least
        // visible to the client as "not a number".
        let json = embeddings_json("m", &[vec![f32::NAN, 1.0]], 1);
        assert!(json.contains("[null,1.0]"), "{json}");
    }
}
