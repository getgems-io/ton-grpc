use crate::client::Error;
use crate::tl::{
    LiteServerBlockData, LiteServerBlockHeader, LiteServerError, LiteServerGetBlock,
    LiteServerLookupBlock, TonNodeBlockId, TonNodeBlockIdExt,
};
use std::borrow::Borrow;
use tower::{Service, ServiceExt};

pub async fn find_first_block_header<S>(
    client: &mut S,
    start: impl Borrow<TonNodeBlockIdExt>,
    lhs: Option<i32>,
    cur: Option<i32>,
) -> Result<LiteServerBlockHeader, Error>
where
    S: Service<LiteServerLookupBlock, Response = LiteServerBlockHeader, Error = Error>,
    S: Service<LiteServerGetBlock, Response = LiteServerBlockData, Error = Error>,
{
    let start = start.borrow();
    let mut rhs = start.seqno;
    let mut lhs = lhs.unwrap_or(1);
    let mut cur = cur.unwrap_or(start.seqno - 200000);

    let workchain = start.workchain;
    let shard = start.shard;

    let mut block = check_block_available(client, TonNodeBlockId::new(workchain, shard, cur)).await;
    let mut success = None;

    let mut hops = 0;

    while lhs < rhs {
        match block {
            Ok(_) => rhs = cur,
            Err(Error::LiteServerError(LiteServerError { code: 651, .. })) => lhs = cur + 1,
            Err(e) => return Err(e),
        }

        cur = (lhs + rhs) / 2;
        if cur == 0 {
            break;
        }

        block = check_block_available(client, TonNodeBlockId::new(workchain, shard, cur)).await;
        if block.is_ok() {
            success = Some(block.as_ref().unwrap().clone());
        }

        hops += 1;
    }

    let delta = 4;
    let (header, _) = match block {
        Ok(b) => b,
        Err(e) => match success {
            Some(b) if b.0.id.seqno - cur <= delta => b,
            _ => return Err(e),
        },
    };

    tracing::trace!(hops = hops, seqno = header.id.seqno, "first seqno");

    Ok(header)
}

async fn check_block_available<S, E>(
    client: &mut S,
    block_id: TonNodeBlockId,
) -> Result<(LiteServerBlockHeader, LiteServerBlockData), E>
where
    S: Service<LiteServerLookupBlock, Response = LiteServerBlockHeader, Error = E>,
    S: Service<LiteServerGetBlock, Response = LiteServerBlockData, Error = E>,
{
    // TODO[akostylev0] research
    let block_header = client
        .oneshot(LiteServerLookupBlock::seqno(block_id))
        .await?;
    let block = client
        .oneshot(LiteServerGetBlock::new(block_header.id.clone()))
        .await?;

    Ok((block_header, block))
}
