use crate::connector::converge::converge;

mod models;
mod r#type;

mod bluefin;
mod connector;
mod env;
mod sockets;

#[tokio::main]
async fn main() {
    converge().await;
}
