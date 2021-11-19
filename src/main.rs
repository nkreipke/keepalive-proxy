use hyper::service::{make_service_fn, service_fn};
use hyper::Server;

mod service;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let localhost = ([127, 0, 0, 1], 9250).into();

    let service =
        make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(service::service)) });

    let server = Server::bind(&localhost).serve(service);

    println!("Proxy server started on http://{}", localhost);

    server.await?;

    Ok(())
}
