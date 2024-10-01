pub mod handler;
pub mod info;
pub mod ledger;
pub mod sub;

use crate::{handler::SlotHandler, ledger::Ledger};
use eyre::Result;
use lumio_p2p::{Config, Node};
use std::sync::Arc;

pub async fn spawn_engine_service<L: Ledger + Send + Sync + 'static>(
    ledger: L,
    p2p_cfg: Config,
    engine_type: EngineType,
) -> Result<()> {
    let ledger = Arc::new(ledger);
    let (mut node, runner) = Node::new(p2p_cfg)?;
    tokio::spawn(runner.run());
    match engine_type {
        EngineType::OpMove => {
            let handler = SlotHandler::new(
                ledger.clone(),
                node.subscribe_lumio_op_move_events(/*ledger.get_committed_l1_slot()?*/).await?,
            );
            tokio::spawn(handler.run());
        }
        EngineType::OpSol => {
            let handler = SlotHandler::new(
                ledger.clone(),
                node.subscribe_lumio_op_sol_events(/*ledger.get_committed_l1_slot()? */).await?,
            );
            tokio::spawn(handler.run());
        }
    };

    // while let Some(request) = node.op_sol_subscribe_request().await? {
    //     let from_slot = request.from_slot;
    //     let (sub_task, stream) = SlotSub::new(from_slot);
    //     tokio::spawn(sub_task.run());
    //     request.sender.send(stream).await?;
    // }

    Ok(())
}

pub enum EngineType {
    OpMove,
    OpSol,
}
