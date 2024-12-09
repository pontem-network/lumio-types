use futures::prelude::*;
use lumio_types::{
    events::{lumio::LumioEvent, Transfer}, h256::H256, p2p::{EngineEvents, LumioEvents}, to::To
};

#[tokio::test]
async fn simple() {
    super::init();
    let (lumio, sol, mv) = super::start().await;

    let mut sol_final = sol.handle_finalize().await.unwrap();
    lumio.op_sol_finalize(10, Default::default()).await.unwrap();
    assert_eq!(sol_final.next().await.unwrap(), (10, Default::default()));

    let mut lumio_attrs = lumio.handle_attrs_since().await.unwrap();
    let mut sol_attrs = sol.subscribe_lumio_attrs_since(10).await.unwrap();
    let (slot, mut lumio_attrs_sub) = lumio_attrs.next().await.unwrap();
    assert_eq!(slot, 10);
     let attrs = LumioEvents::new(
        11,
        vec![To::Lumio(LumioEvent::Sol(Transfer {
            account: H256::default(),
            amount: 1313,
        }))],
    );
    lumio_attrs_sub.send(attrs.clone()).await.unwrap();
    assert_eq!(sol_attrs.next().await.unwrap().unwrap(), attrs);

    let mut mv = mv.subscribe_engine_since(10).await.unwrap();
    let mut op_move_env = sol.handle_engine_since().await.unwrap();
    let (slot, mut sub) = op_move_env.next().await.unwrap();
    assert_eq!(slot, 10);
    let actions = EngineEvents::new(slot, vec![]);
    sub.send(actions.clone()).await.unwrap();

    assert_eq!(mv.next().await.unwrap().unwrap(), actions);
}
