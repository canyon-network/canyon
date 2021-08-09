use futures::executor::block_on;
use libp2p::{build_multiaddr, PeerId};

use sc_block_builder::BlockBuilderProvider;
use sc_client_api::BlockBackend;
use sc_consensus::{import_single_block, BlockImportStatus, ImportedAux, IncomingBlock};
use sc_network_test::PassThroughVerifier;
use sp_consensus::BlockOrigin;
use sp_runtime::generic::BlockId;

use substrate_test_runtime_client::{
    self,
    prelude::*,
    runtime::{Block, Extrinsic, Hash, Transfer},
    TestClient,
};

fn prepare_good_block() -> (TestClient, Hash, u64, PeerId, IncomingBlock<Block>) {
    let mut client = substrate_test_runtime_client::new();
    let block = client
        .new_block(Default::default())
        .unwrap()
        .build()
        .unwrap()
        .block;

    block_on(client.import(BlockOrigin::File, block)).unwrap();

    let (hash, number) = (client.block_hash(1).unwrap().unwrap(), 1);
    let header = client.header(&BlockId::Number(1)).unwrap();
    let justifications = client.justifications(&BlockId::Number(1)).unwrap();
    let peer_id = PeerId::random();
    (
        client,
        hash,
        number,
        peer_id.clone(),
        IncomingBlock {
            hash,
            header,
            body: Some(Vec::new()),
            indexed_body: None,
            justifications,
            origin: Some(peer_id.clone()),
            allow_missing_state: false,
            import_existing: false,
            state: None,
            skip_execution: false,
        },
    )
}

#[test]
fn import_single_good_block_works() {
    let (_, _hash, number, peer_id, block) = prepare_good_block();

    let mut expected_aux = ImportedAux::default();
    expected_aux.is_new_best = true;

    match block_on(import_single_block(
        &mut substrate_test_runtime_client::new(),
        BlockOrigin::File,
        block,
        &mut PassThroughVerifier::new(true),
    )) {
        Ok(BlockImportStatus::ImportedUnknown(ref num, ref aux, ref org))
            if *num == number && *aux == expected_aux && *org == Some(peer_id) => {}
        r @ _ => panic!("{:?}", r),
    }
}
