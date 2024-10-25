use futures::prelude::*;
use lumio_types::p2p::SlotAttribute;

#[tokio::test]
async fn simple() {
    super::init();
    let (lumio, sol, _mv) = super::start().await;

    let mut sol_final = sol.handle_finalize().await.unwrap();
    lumio.op_sol_finalize(10, Default::default()).await.unwrap();
    assert_eq!(sol_final.next().await.unwrap(), (10, Default::default()));

    let mut lumio_attrs = lumio.handle_attrs_since().await.unwrap();
    let mut sol_attrs = sol.subscribe_lumio_attrs_since(10).await.unwrap();
    let (slot, mut lumio_attrs_sub) = lumio_attrs.next().await.unwrap();
    assert_eq!(slot, 10);
    let attrs = SlotAttribute {
        events: vec![],
        slot_id: 11,
        sync_status: None,
    };
    lumio_attrs_sub.send(attrs.clone()).await.unwrap();
    assert_eq!(sol_attrs.next().await.unwrap().unwrap(), attrs);

    let mut lumio_attrs = lumio.handle_attrs_since().await.unwrap();
    let mut sol_attrs = sol.subscribe_lumio_attrs_since(10).await.unwrap();
    let (slot, mut lumio_attrs_sub) = lumio_attrs.next().await.unwrap();
    assert_eq!(slot, 10);
    let attrs = SlotAttribute {
        events: vec![],
        slot_id: 11,
        sync_status: None,
    };
    lumio_attrs_sub.send(attrs.clone()).await.unwrap();
    assert_eq!(sol_attrs.next().await.unwrap().unwrap(), attrs);
}
