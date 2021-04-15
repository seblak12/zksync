//! Fee part of API implementation.

// Built-in uses

// External uses
use actix_web::{
    web::{self, Json},
    Scope,
};

// Workspace uses
use zksync_api_types::v02::fee::{ApiBatchFee, ApiFee, BatchFeeRequest, TxFeeRequest};

// Local uses
use super::{error::Error, response::ApiResult};
use crate::api_server::tx_sender::TxSender;

/// Shared data between `api/v0.2/fee` endpoints.
#[derive(Clone)]
struct ApiFeeData {
    tx_sender: TxSender,
}

impl ApiFeeData {
    fn new(tx_sender: TxSender) -> Self {
        Self { tx_sender }
    }
}

async fn get_tx_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<TxFeeRequest>,
) -> ApiResult<ApiFee> {
    data.tx_sender
        .get_txs_fee_in_wei(body.tx_type, body.address, body.token_like)
        .await
        .map_err(Error::from)
        .map(ApiFee::from)
        .into()
}

async fn get_batch_fee(
    data: web::Data<ApiFeeData>,
    Json(body): Json<BatchFeeRequest>,
) -> ApiResult<ApiBatchFee> {
    let mut txs = Vec::new();
    for tx in body.transactions {
        txs.push((tx.tx_type, tx.address));
    }
    data.tx_sender
        .get_txs_batch_fee_in_wei(txs, body.token_like)
        .await
        .map_err(Error::from)
        .map(ApiBatchFee::from)
        .into()
}

pub fn api_scope(tx_sender: TxSender) -> Scope {
    let data = ApiFeeData::new(tx_sender);

    web::scope("fee")
        .data(data)
        .route("", web::post().to(get_tx_fee))
        .route("/batch", web::post().to(get_batch_fee))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_server::rest::v02::{
        test_utils::{
            deserialize_response_result, dummy_fee_ticker, dummy_sign_verifier, TestServerConfig,
        },
        SharedData,
    };
    use num::BigUint;
    use zksync_api_types::v02::{fee::TxInBatchFeeRequest, ApiVersion};
    use zksync_types::{tokens::TokenLike, Address, OutputFeeType, TokenId, TxFeeTypes};

    #[actix_rt::test]
    #[cfg_attr(
        not(feature = "api_test"),
        ignore = "Use `zk test rust-api` command to perform this test"
    )]
    async fn v02_test_fee_scope() -> anyhow::Result<()> {
        let cfg = TestServerConfig::default();

        let shared_data = SharedData {
            net: cfg.config.chain.eth.network,
            api_version: ApiVersion::V02,
        };
        let (client, server) = cfg.start_server(
            move |cfg: &TestServerConfig| {
                api_scope(TxSender::new(
                    cfg.pool.clone(),
                    dummy_sign_verifier(),
                    dummy_fee_ticker(&[]),
                    &cfg.config,
                ))
            },
            shared_data,
        );

        let tx_type = TxFeeTypes::Withdraw;
        let address = Address::default();
        let token_like = TokenLike::Id(TokenId(1));

        let response = client
            .get_txs_fee_v02(tx_type, address, token_like.clone())
            .await?;
        let api_fee: ApiFee = deserialize_response_result(response)?;
        assert_eq!(api_fee.fee_type, OutputFeeType::Withdraw);
        assert_eq!(api_fee.gas_tx_amount, BigUint::from(1u32));
        assert_eq!(api_fee.gas_price_wei, BigUint::from(1u32));
        assert_eq!(api_fee.gas_fee, BigUint::from(1u32));
        assert_eq!(api_fee.zkp_fee, BigUint::from(1u32));
        assert_eq!(api_fee.total_fee, BigUint::from(2u32));

        let tx = TxInBatchFeeRequest {
            tx_type: TxFeeTypes::Withdraw,
            address: Address::default(),
        };
        let txs = vec![tx.clone(), tx.clone(), tx];

        let response = client.get_batch_fee_v02(txs, token_like).await?;
        let api_batch_fee: ApiBatchFee = deserialize_response_result(response)?;
        assert_eq!(api_batch_fee.total_fee, BigUint::from(6u32));

        server.stop().await;
        Ok(())
    }
}
