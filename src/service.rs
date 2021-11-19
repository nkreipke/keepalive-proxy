use hyper::client::HttpConnector;
use hyper::http::uri::Scheme;
use hyper::http::HeaderValue;
use hyper::upgrade::Upgraded;
use hyper::{Body, Client, Method, Request, Response, StatusCode};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::spawn;

lazy_static::lazy_static! {
    static ref CLIENT: Client<HttpConnector> = Client::builder()
        .pool_idle_timeout(Duration::from_secs(5))
        .build_http();
}

pub async fn service(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let method = req.method();
    let uri = req.uri();

    // For HTTPS requests, we cannot modify requests in any way, so we simply connect them through.
    // This assumes that the initial requests might be HTTPS, but the bulk of streaming data is then
    // transferred over HTTP.
    if method == Method::CONNECT {
        return connect_directly(req).await;
    }

    // We only accept GET requests
    if method != Method::GET {
        eprintln!("Invalid method: {}", method);
        return Ok(error_response(
            "Method not supported (keepalive-proxy)",
            StatusCode::BAD_REQUEST,
        ));
    }

    // We only accept absolute URIs with an HTTP scheme
    if uri.scheme() != Some(&Scheme::HTTP) {
        eprintln!("Invalid URI scheme: {}", uri);
        return Ok(error_response(
            "Invalid URI scheme or not an absolute URI (keepalive-proxy)",
            StatusCode::BAD_REQUEST,
        ));
    }

    println!("Proxy request for {}", uri);

    // Upon returning the response, the body will be asynchronously streamed to the client
    connect_proxy(req).await
}

async fn connect_proxy(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    // Build a proxy request
    let mut proxy_req = Request::builder().method(Method::GET).uri(req.uri());

    // Copy headers
    for (name, value) in req.headers() {
        if name == "Proxy-Connection" || name == "Connection" {
            // Skip connection headers. This does not strictly conform to RFC 7230 3.2.1
            // but it works for now. To make this compliant, we also need to remove the headers specified
            // in the value of the "Connection" header.
            continue;
        }

        proxy_req = proxy_req.header(name, value);
    }

    let proxy_req = match proxy_req
        // Change the connection type to keep-alive to keep this connection open
        .header("Connection", HeaderValue::from_str("Keep-Alive").unwrap())
        .body(req.into_body())
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Could not build proxy request: {}", e);
            return Ok(error_response(
                "Error building proxy request",
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    // Start the request
    let mut response = CLIENT.clone().request(proxy_req).await?;

    // Change the connection type to "close" to not confuse the client
    let response_headers = response.headers_mut();
    response_headers.remove("Connection");
    response_headers.insert("Connection", HeaderValue::from_str("close").unwrap());

    Ok(response)
}

async fn connect_directly(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    // Directly connect to the target
    let uri = req.uri();

    let addr = match (uri.host(), uri.port()) {
        (Some(host), Some(port)) => (host, port.as_u16()),
        _ => {
            return Ok(error_response(
                "Invalid URI for CONNECT request",
                StatusCode::BAD_REQUEST,
            ))
        }
    };

    println!(
        "CONNECT request to {}:{} (this connection will not be kept alive!)",
        addr.0, addr.1
    );

    // Connect to the target
    match TcpStream::connect(addr).await {
        Ok(stream) => {
            // Upgrade the connection
            let mut res = Response::new(Body::empty());
            *res.status_mut() = StatusCode::OK;

            spawn(async move {
                match hyper::upgrade::on(req).await {
                    Ok(upgraded) => {
                        if let Err(e) = run_upgraded(upgraded, stream).await {
                            eprintln!("Upgraded connection error: {}", e);
                        }
                    }
                    Err(e) => eprintln!("Connection upgrade failed: {}", e),
                }
            });

            Ok(res)
        }
        Err(e) => {
            eprintln!("Connection failure: {}", e);

            Ok(error_response(
                "Connection to target failed",
                StatusCode::BAD_GATEWAY,
            ))
        }
    }
}

async fn run_upgraded(
    mut upgraded: Upgraded,
    mut stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Simply copy all data to the respective peer
    tokio::io::copy_bidirectional(&mut upgraded, &mut stream).await?;

    println!("Upgraded connection closed");

    Ok(())
}

fn error_response(error: &'static str, code: StatusCode) -> Response<Body> {
    let mut res = Response::new(Body::from(error));

    *res.status_mut() = code;

    res
}
