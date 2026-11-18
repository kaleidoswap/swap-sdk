//! Test price source; all wallet, chain, swap and Lightning operations are real.
use anyhow::Result;
use maker_connector_grpc::marketdata::{
    price_feed_provider_server::{PriceFeedProvider, PriceFeedProviderServer},
    Capabilities, CapabilitiesRequest, SubscribeTickersRequest, Symbol, TickerUpdate,
};
use tokio_stream::wrappers::ReceiverStream;

struct FixedPrice;
#[tonic::async_trait]
impl PriceFeedProvider for FixedPrice {
    async fn get_capabilities(
        &self,
        _: tonic::Request<CapabilitiesRequest>,
    ) -> Result<tonic::Response<Capabilities>, tonic::Status> {
        Ok(tonic::Response::new(Capabilities {
            provider: "rgb-regtest".into(),
            version: "1".into(),
            symbols: vec![Symbol {
                symbol: "BTC/USDT".into(),
                base: "BTC".into(),
                quote: "USDT".into(),
            }],
        }))
    }
    type SubscribeTickersStream = ReceiverStream<Result<TickerUpdate, tonic::Status>>;
    async fn subscribe_tickers(
        &self,
        _: tonic::Request<SubscribeTickersRequest>,
    ) -> Result<tonic::Response<Self::SubscribeTickersStream>, tonic::Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let mut sequence = 0;
            loop {
                sequence += 1;
                // The maker intentionally ages out an unchanged quote, even with fresh
                // timestamps. A tiny monotonic step also avoids sampling the same
                // price on two rate-card refreshes.
                let price = format!("100000.{sequence:06}");
                let update = TickerUpdate {
                    provider: "rgb-regtest".into(),
                    symbol: "BTC/USDT".into(),
                    bid: price.clone(),
                    ask: price,
                    as_of_unix_ms: (super::support::now() * 1000) as i64,
                    sequence,
                    sources: vec![],
                    bids: vec![],
                    asks: vec![],
                    vwap: vec![],
                };
                if tx.send(Ok(update)).await.is_err() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}
pub async fn start() -> Result<tokio::task::JoinHandle<()>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:29421").await?;
    Ok(tokio::spawn(async move {
        let _ = tonic::transport::Server::builder()
            .add_service(PriceFeedProviderServer::new(FixedPrice))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await;
    }))
}
