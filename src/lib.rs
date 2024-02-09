use bitcoin::{address::NetworkUnchecked, Address, Amount, BlockHash, Txid};
use bitcoincore_rpc::{
    bitcoincore_rpc_json::GetTransactionResult,
    json::{AddressType, GetBalancesResult, ListTransactionResult},
    RpcApi,
};
use std::{future::Future, sync::Arc};
use tracing::Instrument;

pub use bitcoincore_rpc as rpc;

#[derive(Clone)]
pub struct Btc {
    client: Arc<bitcoincore_rpc::Client>,
}

pub fn build(rpc_user: &str, rpc_password: &str, rpc_url: &str) -> bitcoincore_rpc::Result<Btc> {
    let auth = bitcoincore_rpc::Auth::UserPass(rpc_user.into(), rpc_password.into());

    bitcoincore_rpc::Client::new(rpc_url, auth).map(|client| Btc {
        client: Arc::new(client),
    })
}

impl Btc {
    pub fn get_block_count(&self) -> impl Future<Output = bitcoincore_rpc::Result<u64>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || client.get_block_count())
                .await
                .unwrap()
        }
        .instrument(span!("get_block_count"))
    }

    pub fn list_transactions(
        &self,
        count: usize,
    ) -> impl Future<Output = bitcoincore_rpc::Result<Vec<ListTransactionResult>>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || {
                client.list_transactions(Some("*"), Some(count), Some(0), None)
            })
            .await
            .unwrap()
        }
        .instrument(span!("listtransactions"))
    }

    pub fn list_since_block(
        &self,
        block_hash: Option<BlockHash>,
        confirmations: usize,
    ) -> impl Future<Output = bitcoincore_rpc::Result<(Vec<ListTransactionResult>, BlockHash)>>
    {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || {
                client
                    .list_since_block(block_hash.as_ref(), Some(confirmations), None, None)
                    .map(|outcome| (outcome.transactions, outcome.lastblock))
            })
            .await
            .unwrap()
        }
        .instrument(span!("listsinceblock"))
    }

    pub fn get_balances(&self) -> impl Future<Output = bitcoincore_rpc::Result<GetBalancesResult>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || client.get_balances())
                .await
                .unwrap()
        }
        .instrument(span!("getbalances"))
    }

    pub fn get_balance(
        &self,
        number_of_confirmations: Option<usize>,
    ) -> impl Future<Output = bitcoincore_rpc::Result<Amount>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || client.get_balance(number_of_confirmations, None))
                .await
                .unwrap()
        }
        .instrument(span!("getbalance"))
    }

    pub fn get_transaction(
        &self,
        txid: Txid,
    ) -> impl Future<Output = bitcoincore_rpc::Result<GetTransactionResult>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || client.get_transaction(&txid, Some(true)))
                .await
                .unwrap()
        }
        .instrument(span!("gettransaction"))
    }

    pub fn generate_address(
        &self,
        address_type: AddressType,
    ) -> impl Future<Output = bitcoincore_rpc::Result<Address<NetworkUnchecked>>> {
        let client = self.client.clone();

        async move {
            tokio::task::spawn_blocking(move || client.get_new_address(None, Some(address_type)))
                .await
                .unwrap()
        }
        .instrument(span!("getnewaddress"))
    }
}

#[macro_export]
macro_rules! span {
    ($method:literal) => {
        tracing::info_span!(
            "btcore",
            service.name = "btcore",
            otel.name = $method,
            otel.kind = "client",
            rpc.system = "jsonrpc",
            rpc.service = "bitcoind",
            rpc.method = $method,
        )
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use bitcoincore_rpc::{bitcoincore_rpc_json::LoadWalletResult, RpcApi};
    use std::{str::FromStr, sync::Arc};

    async fn build_for_test() -> bitcoincore_rpc::Result<Btc> {
        const USERNAME: &str = "dev";
        const PASSWORD: &str = "dev";
        const RPC_URL: &str = "http://0.0.0.0:18443/";
        const WALLET_NAME: &str = "RegtestWallet";

        let auth = bitcoincore_rpc::Auth::UserPass(USERNAME.into(), PASSWORD.into());
        let client = bitcoincore_rpc::Client::new(RPC_URL, auth).map(|client| Btc {
            client: Arc::new(client),
        })?;

        client.reset_regtest(WALLET_NAME).await?;

        Ok(client)
    }

    impl Btc {
        async fn create_wallet<'a>(
            &self,
            wallet_name: &'a str,
        ) -> bitcoincore_rpc::Result<LoadWalletResult> {
            let client = self.client.clone();
            let wallet_name = wallet_name.to_owned();

            tokio::task::spawn_blocking(move || {
                client.create_wallet(&wallet_name, None, None, None, None)
            })
            .await
            .unwrap()
        }

        async fn unload_wallet<'a>(&self, wallet_name: &'a str) -> bitcoincore_rpc::Result<()> {
            let client = self.client.clone();
            let wallet_name = wallet_name.to_owned();

            tokio::task::spawn_blocking(move || client.unload_wallet(Some(&wallet_name)))
                .await
                .unwrap()
        }

        fn delete_wallet(&self, wallet_name: &str) -> Result<(), std::io::Error> {
            use std::fs;

            let path = ".btc/regtest/wallets/".to_owned() + &wallet_name;

            fs::remove_dir_all(path)
        }

        pub async fn reset_regtest(&self, wallet_name: &str) -> Result<(), bitcoincore_rpc::Error> {
            self.unload_wallet(wallet_name).await.ok();
            self.delete_wallet(wallet_name).ok();
            self.invalidate_all_blocks().await;
            self.create_wallet(wallet_name.into()).await?;

            Ok(())
        }

        pub async fn get_best_block_hash(&self) -> bitcoincore_rpc::Result<BlockHash> {
            let client = self.client.clone();

            tokio::task::spawn_blocking(move || client.get_best_block_hash())
                .await
                .unwrap()
        }

        async fn invalidate_all_blocks(&self) {
            let client = self.client.clone();
            let mut current_block_hash = self.get_best_block_hash().await.unwrap();

            tokio::task::spawn_blocking(move || {
                let current_block_height = client.get_block_count().unwrap();

                for _ in (0..current_block_height).rev() {
                    client
                        .invalidate_block(&current_block_hash.clone())
                        .unwrap();

                    let previous_block_hash = client
                        .get_block_header(&current_block_hash)
                        .unwrap()
                        .prev_blockhash;

                    current_block_hash = previous_block_hash;
                }
            })
            .await
            .unwrap();
        }

        /// Generates `block_num` blocks and sends the block subsidy to an address.
        /// The `address` parameter is optional, allowing for the "burning" of the subsidy by sending it
        /// to an address not contained in the wallet.
        ///
        /// # Arguments
        ///
        /// * `block_num`: The number of blocks to generate.
        /// * `address`: The address to which the coins will be sent.
        pub async fn generate_n_blocks(
            &self,
            block_num: u64,
            burn_coins: bool,
        ) -> bitcoincore_rpc::Result<Vec<bitcoin::BlockHash>> {
            let client = self.client.clone();

            let address = if burn_coins {
                Address::from_str("bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw").unwrap()
            } else {
                self.generate_address_async(AddressType::P2shSegwit)
                    .await
                    .unwrap()
            };

            tokio::task::spawn_blocking(move || client.generate_to_address(block_num, &address))
                .await
                .unwrap()
        }

        /// In Bitcoin, block subsidies are "locked" for 100 blocks, which means that the first subsidy can
        /// only be spent after 100 confirmations. This function mines 101 blocks and sends the subsidy of
        /// the first block to an address controlled by the wallet, allowing for immediate spending.
        pub async fn generate_one_spendable_output(&self) -> bitcoincore_rpc::Result<()> {
            self.generate_n_blocks(1, false).await?;
            self.generate_n_blocks(100, true).await?;

            Ok(())
        }
    }

    #[tokio::test]
    async fn test_get_balance() {
        let client = build_for_test().await.unwrap();
        let balance = client.get_balance(None).await.unwrap();

        assert_eq!(balance, bitcoin::Amount::ZERO);

        client.generate_one_spendable_output().await.unwrap();

        let amount = client.get_balance(None).await.unwrap();

        assert_eq!(amount, bitcoin::Amount::from_btc(50.0).unwrap());
    }

    #[tokio::test]
    async fn test_list_transactions() {
        let client = build_for_test().await.unwrap();
        let number_of_transactions = client.list_transactions(1000).await.unwrap().len();

        assert_eq!(number_of_transactions, 0);

        client.generate_n_blocks(101, false).await.unwrap();

        let number_of_transactions = client.list_transactions(1000).await.unwrap().len();

        assert_eq!(number_of_transactions, 101);
    }

    #[tokio::test]
    async fn test_list_since_block() {
        let client = build_for_test().await.unwrap();
        let (transactions, _) = client.list_since_block(None, 1).await.unwrap();

        assert_eq!(transactions.len(), 0); // No transactions should return 0

        client.generate_n_blocks(5, true).await.unwrap();

        let (transactions, _) = client.list_since_block(None, 1).await.unwrap();

        assert_eq!(transactions.len(), 0); // Chain has txs but not concerning this wallet

        client.generate_n_blocks(1, false).await.unwrap();

        let (transactions, _) = client.list_since_block(None, 1).await.unwrap();

        assert_eq!(transactions.len(), 1); // Chain has 1 tx confirmed concerning an wallet address

        let blocks = client.generate_n_blocks(1, true).await.unwrap();
        let block_hash = blocks.last().unwrap();

        let (transactions, _) = client.list_since_block(Some(*block_hash), 1).await.unwrap();

        assert_eq!(transactions.len(), 0) // No seen tx with 1 conf since block_hash
    }

    #[tokio::test]
    async fn test_get_balances() {
        let client = build_for_test().await.unwrap();
        let balances = client.get_balances().await.unwrap();

        assert_eq!(balances.mine.trusted, Amount::ZERO);

        client.generate_one_spendable_output().await.unwrap();

        let balances = client.get_balances().await.unwrap();

        assert_eq!(balances.mine.trusted, Amount::from_btc(50.0).unwrap());

        client.invalidate_all_blocks().await;

        let balances = client.get_balances().await.unwrap();

        assert_eq!(balances.mine.trusted, Amount::ZERO);
    }

    #[tokio::test]
    async fn test_get_transaction_fee() {
        let client = build_for_test().await.unwrap();

        client.generate_one_spendable_output().await.unwrap();

        let fee_rate = 1 as i64;

        let txid = client
            .send_to_address(
                client
                    .generate_address_async(AddressType::P2shSegwit)
                    .await
                    .unwrap(),
                5_000_000,
                Some(fee_rate as i32),
            )
            .await
            .unwrap();

        client.generate_n_blocks(1, true).await.unwrap();

        let fee = client.get_transaction_fee(txid).await.unwrap().unwrap();
        let expected_fee = -(fee.vsize as i64 * fee_rate);

        assert_eq!(fee.fee, expected_fee);
    }
}
