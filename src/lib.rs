use bitcoin::{Address, Amount, BlockHash, Txid};
use bitcoincore_rpc::{
    json::{AddressType, GetBalancesResult, ListTransactionResult},
    RpcApi,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct Btc {
    client: Arc<bitcoincore_rpc::Client>,
}

pub struct BtcParams {
    rpc_user: String,
    rpc_password: String,
    rpc_url: String,
}

pub fn build(params: BtcParams) -> bitcoincore_rpc::Result<Btc> {
    let auth = bitcoincore_rpc::Auth::UserPass(params.rpc_user, params.rpc_password);

    bitcoincore_rpc::Client::new(&params.rpc_url, auth).map(|client| Btc {
        client: Arc::new(client),
    })
}

impl Btc {
    pub async fn list_transactions(
        &self,
        count: usize,
    ) -> bitcoincore_rpc::Result<Vec<ListTransactionResult>> {
        let client = self.client.clone();

        tokio::task::spawn_blocking(move || {
            client.list_transactions(Some("*"), Some(count), Some(0), None)
        })
        .await
        .unwrap()
    }

    pub async fn list_since_block(
        &self,
        block_hash: Option<BlockHash>,
        confirmations: usize,
    ) -> bitcoincore_rpc::Result<(Vec<ListTransactionResult>, BlockHash)> {
        let client = self.client.clone();

        tokio::task::spawn_blocking(move || {
            client
                .list_since_block(block_hash.as_ref(), Some(confirmations), None, None)
                .map(|outcome| (outcome.transactions, outcome.lastblock))
        })
        .await
        .unwrap()
    }

    pub async fn get_balances(&self) -> bitcoincore_rpc::Result<GetBalancesResult> {
        let client = self.client.clone();
        let res = tokio::task::spawn_blocking(move || client.get_balances());
        res.await.unwrap()
    }

    pub async fn get_balance(
        &self,
        number_of_confirmations: Option<usize>,
    ) -> bitcoincore_rpc::Result<Amount> {
        let client = self.client.clone();
        let res =
            tokio::task::spawn_blocking(move || client.get_balance(number_of_confirmations, None));
        res.await.unwrap()
    }

    pub async fn get_transaction_fee(
        &self,
        txid: Txid,
    ) -> bitcoincore_rpc::Result<Option<TransactionFee>> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            let transaction = client.get_transaction(&txid, Some(true))?;

            let Some((fee, blockhash)) = transaction.fee.zip(transaction.info.blockhash) else {
                return Ok(None);
            };

            let raw_transaction = client.get_raw_transaction_info(&txid, Some(&blockhash))?;

            Ok(Some(TransactionFee {
                vsize: raw_transaction.vsize,
                fee: fee.to_sat(),
            }))
        })
        .await
        .unwrap()
    }

    pub async fn send_to_address(
        &self,
        address: Address,
        amount_satoshi: i64,
        fee_rate: Option<i32>,
    ) -> bitcoincore_rpc::Result<bitcoin::Txid> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            client.send_to_address(
                &address,
                Amount::from_sat(amount_satoshi as u64),
                Some(""),
                Some(""),
                Some(true),
                Some(true),
                None,
                None,
                None,
                fee_rate,
            )
        })
        .await
        .unwrap()
    }

    pub async fn send_many(
        &self,
        addresses: HashMap<Address, Amount>,
        fee_rate: i32,
    ) -> bitcoincore_rpc::Result<bitcoin::Txid> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            client.send_many(
                addresses,
                Some(""),
                None,
                Some(true),
                None,
                None,
                Some(fee_rate),
            )
        })
        .await
        .unwrap()
    }

    /// DANGEROUS: this call will block the thread. it is not safe unless you know what you're doing.
    pub fn generate_address_blocking(
        &self,
        address_type: AddressType,
    ) -> bitcoincore_rpc::Result<Address> {
        self.client.get_new_address(None, Some(address_type))
    }

    pub async fn generate_address_async(
        &self,
        address_type: AddressType,
    ) -> bitcoincore_rpc::Result<Address> {
        let client = self.client.clone();

        tokio::task::spawn_blocking(move || client.get_new_address(None, Some(address_type)))
            .await
            .unwrap()
    }
}

pub struct TransactionFee {
    pub vsize: usize,
    pub fee: i64,
}
