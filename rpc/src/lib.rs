use std::sync::Arc;

use primitives::{AccountId, Balance, Block, BlockNumber, CurrencyId, DataProviderId, Hash, Nonce};
use sc_client_api::light::{Fetcher, RemoteBlockchain};
use sc_consensus_babe::Epoch;
use sc_finality_grandpa::{FinalityProofProvider, GrandpaJustificationStream, SharedAuthoritySet, SharedVoterState};
pub use sc_rpc::DenyUnsafe;
pub use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_transaction_pool::TransactionPool;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Light client extra dependencies.
pub struct LightDeps<C, F, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Remote access to the blockchain (async).
	pub remote_blockchain: Arc<dyn RemoteBlockchain<Block>>,
	/// Fetcher instance.
	pub fetcher: Arc<F>,
}

/// Extra dependencies for BABE.
pub struct BabeDeps {
	/// BABE protocol config.
	pub babe_config: sc_consensus_babe::Config,
	/// BABE pending epoch changes.
	pub shared_epoch_changes: sc_consensus_epochs::SharedEpochChanges<Block, Epoch>,
	/// The keystore that manages the keys of the node.
	pub keystore: sc_keystore::KeyStorePtr,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
	/// Voting round info.
	pub shared_voter_state: SharedVoterState,
	/// Authority set info.
	pub shared_authority_set: SharedAuthoritySet<Hash, BlockNumber>,
	/// Receives notifications about justification events from Grandpa.
	pub justification_stream: GrandpaJustificationStream<Block>,
	/// Subscription manager to keep track of pubsub subscribers.
	pub subscription_executor: SubscriptionTaskExecutor,
	/// Finality proof provider.
	pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Full client dependencies.
pub struct FullDeps<C, P, SC, B> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// The SelectChain Strategy
	pub select_chain: SC,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// BABE specific dependencies.
	pub babe: BabeDeps,
	/// GRANDPA specific dependencies.
	pub grandpa: GrandpaDeps<B>,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, SC, B>(deps: FullDeps<C, P, SC, B>) -> RpcExtension
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, DataProviderId, CurrencyId, dev_runtime::TimeStampedPrice>,
	C::Api: margin_protocol_rpc::MarginProtocolRuntimeApi<Block, AccountId>,
	C::Api: synthetic_protocol_rpc::SyntheticProtocolRuntimeApi<Block, AccountId>,
	C::Api: BabeApi<Block>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + Sync + Send + 'static,
	SC: SelectChain<Block> + 'static,
	B: sc_client_api::Backend<Block> + Send + Sync + 'static,
	B::State: sc_client_api::StateBackend<sp_runtime::traits::HashFor<Block>>,
{
	use margin_protocol_rpc::{MarginProtocol, MarginProtocolApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use sc_consensus_babe_rpc::BabeRpcHandler;
	use sc_finality_grandpa_rpc::{GrandpaApi, GrandpaRpcHandler};
	use substrate_frame_rpc_system::{FullSystem, SystemApi};
	use synthetic_protocol_rpc::{SyntheticProtocol, SyntheticProtocolApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		pool,
		select_chain,
		deny_unsafe,
		babe,
		grandpa,
	} = deps;
	let BabeDeps {
		keystore,
		babe_config,
		shared_epoch_changes,
	} = babe;
	let GrandpaDeps {
		shared_voter_state,
		shared_authority_set,
		justification_stream,
		subscription_executor,
		finality_provider,
	} = grandpa;

	io.extend_with(SystemApi::to_delegate(FullSystem::new(
		client.clone(),
		pool,
		deny_unsafe,
	)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	io.extend_with(sc_consensus_babe_rpc::BabeApi::to_delegate(BabeRpcHandler::new(
		client.clone(),
		shared_epoch_changes,
		keystore,
		babe_config,
		select_chain,
		deny_unsafe,
	)));
	io.extend_with(GrandpaApi::to_delegate(GrandpaRpcHandler::new(
		shared_authority_set,
		shared_voter_state,
		justification_stream,
		subscription_executor,
		finality_provider,
	)));
	io.extend_with(OracleApi::to_delegate(Oracle::new(client.clone())));
	io.extend_with(MarginProtocolApi::to_delegate(MarginProtocol::new(client.clone())));
	io.extend_with(SyntheticProtocolApi::to_delegate(SyntheticProtocol::new(client)));

	io
}

/// Instantiate all RPC extensions for light node.
pub fn create_light<C, P, F>(deps: LightDeps<C, F, P>) -> RpcExtension
where
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	F: Fetcher<Block> + 'static,
	P: TransactionPool + 'static,
{
	use substrate_frame_rpc_system::{LightSystem, SystemApi};

	let LightDeps {
		client,
		pool,
		remote_blockchain,
		fetcher,
	} = deps;
	let mut io = jsonrpc_core::IoHandler::default();
	io.extend_with(SystemApi::<Hash, AccountId, Nonce>::to_delegate(LightSystem::new(
		client,
		remote_blockchain,
		fetcher,
		pool,
	)));

	io
}
