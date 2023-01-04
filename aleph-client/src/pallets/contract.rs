use codec::{Compact, Encode};
use pallet_contracts_primitives::ContractExecResult;
use primitives::Balance;
use subxt::{ext::sp_core::Bytes, rpc_params};

use crate::{
    api, pallet_contracts::wasm::OwnerInfo, sp_weights::weight_v2::Weight, AccountId, BlockHash,
    ConnectionApi, SignedConnectionApi, TxStatus,
};

#[derive(Encode)]
pub struct ContractCallArgs {
    pub origin: AccountId,
    pub dest: AccountId,
    pub value: Balance,
    pub gas_limit: Option<Weight>,
    pub storage_deposit_limit: Option<Balance>,
    pub input_data: Vec<u8>,
}

#[async_trait::async_trait]
pub trait ContractsApi {
    async fn get_owner_info(
        &self,
        code_hash: BlockHash,
        at: Option<BlockHash>,
    ) -> Option<OwnerInfo>;
}

#[async_trait::async_trait]
pub trait ContractsUserApi {
    async fn upload_code(
        &self,
        code: Vec<u8>,
        storage_limit: Option<Compact<u128>>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    #[allow(clippy::too_many_arguments)]
    async fn instantiate(
        &self,
        code_hash: BlockHash,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    #[allow(clippy::too_many_arguments)]
    async fn instantiate_with_code(
        &self,
        code: Vec<u8>,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn call(
        &self,
        destination: AccountId,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
    async fn remove_code(
        &self,
        code_hash: BlockHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash>;
}

#[async_trait::async_trait]
pub trait ContractRpc {
    async fn call_and_get(
        &self,
        args: ContractCallArgs,
    ) -> anyhow::Result<ContractExecResult<Balance>>;
}

#[async_trait::async_trait]
impl<C: ConnectionApi> ContractsApi for C {
    async fn get_owner_info(
        &self,
        code_hash: BlockHash,
        at: Option<BlockHash>,
    ) -> Option<OwnerInfo> {
        let addrs = api::storage().contracts().owner_info_of(code_hash);

        self.get_storage_entry_maybe(&addrs, at).await
    }
}

#[async_trait::async_trait]
impl<S: SignedConnectionApi> ContractsUserApi for S {
    async fn upload_code(
        &self,
        code: Vec<u8>,
        storage_limit: Option<Compact<u128>>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().contracts().upload_code(code, storage_limit);

        self.send_tx(tx, status).await
    }

    async fn instantiate(
        &self,
        code_hash: BlockHash,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().contracts().instantiate(
            balance,
            gas_limit,
            storage_limit,
            code_hash,
            data,
            salt,
        );

        self.send_tx(tx, status).await
    }

    async fn instantiate_with_code(
        &self,
        code: Vec<u8>,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        salt: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().contracts().instantiate_with_code(
            balance,
            gas_limit,
            storage_limit,
            code,
            data,
            salt,
        );

        self.send_tx(tx, status).await
    }

    async fn call(
        &self,
        destination: AccountId,
        balance: Balance,
        gas_limit: Weight,
        storage_limit: Option<Compact<u128>>,
        data: Vec<u8>,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx =
            api::tx()
                .contracts()
                .call(destination.into(), balance, gas_limit, storage_limit, data);
        self.send_tx(tx, status).await
    }

    async fn remove_code(
        &self,
        code_hash: BlockHash,
        status: TxStatus,
    ) -> anyhow::Result<BlockHash> {
        let tx = api::tx().contracts().remove_code(code_hash);

        self.send_tx(tx, status).await
    }
}

#[async_trait::async_trait]
impl<C: ConnectionApi> ContractRpc for C {
    async fn call_and_get(
        &self,
        args: ContractCallArgs,
    ) -> anyhow::Result<ContractExecResult<Balance>> {
        let params = rpc_params!["ContractsApi_call", Bytes(args.encode())];
        self.rpc_call("state_call".to_string(), params).await
    }
}