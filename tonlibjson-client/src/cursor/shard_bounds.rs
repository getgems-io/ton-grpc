use crate::block::BlocksHeader;

#[derive(Debug, Clone, Default)]
pub struct ShardBounds {
    pub left: Option<BlocksHeader>,
    pub right: Option<BlocksHeader>,
    pub right_end: Option<crate::cursor::Seqno>,
}

impl ShardBounds {
    pub fn left(left: BlocksHeader) -> Self {
        Self {
            left: Some(left),
            right: None,
            right_end: None,
        }
    }

    pub fn right(right: BlocksHeader) -> Self {
        Self {
            left: None,
            right_end: Some(right.id.seqno),
            right: Some(right),
        }
    }

    pub fn right_end(right_end: crate::cursor::Seqno) -> Self {
        Self {
            left: None,
            right_end: Some(right_end),
            right: None,
        }
    }

    pub fn right_next(&self) -> Option<crate::cursor::Seqno> {
        let seqno = self.right_end?;

        match self.right {
            None => Some(seqno),
            Some(ref right) if right.id.seqno < seqno => Some(right.id.seqno + 1),
            _ => None,
        }
    }

    pub fn contains_seqno(&self, seqno: crate::cursor::Seqno, not_available: bool) -> bool {
        let Some(ref left) = self.left else {
            return false;
        };
        let Some(ref right) = self.right else {
            return false;
        };

        if not_available {
            left.id.seqno <= seqno && seqno <= self.right_end.unwrap_or(right.id.seqno)
        } else {
            left.id.seqno <= seqno && seqno <= right.id.seqno
        }
    }

    pub fn contains_lt(&self, lt: i64, not_available: bool) -> bool {
        let Some(ref left) = self.left else {
            return false;
        };
        let Some(ref right) = self.right else {
            return false;
        };

        if not_available {
            left.start_lt <= lt && lt <= right.end_lt + (right.end_lt - right.start_lt)
        } else {
            left.start_lt <= lt && lt <= right.end_lt
        }
    }
}
