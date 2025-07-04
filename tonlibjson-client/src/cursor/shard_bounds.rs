use crate::block::BlocksHeader;
use crate::cursor::Seqno;

#[derive(Debug, Clone, Default)]
pub struct ShardBounds {
    left: Option<BlocksHeader>,
    right: Option<BlocksHeader>,
    right_end: Option<Seqno>,
}

impl ShardBounds {
    pub fn from_left(left: BlocksHeader) -> Self {
        Self {
            left: Some(left),
            right: None,
            right_end: None,
        }
    }

    pub fn from_right(right: BlocksHeader) -> Self {
        Self {
            left: None,
            right_end: Some(right.id.seqno),
            right: Some(right),
        }
    }

    pub fn from_right_seqno(right_end: Seqno) -> Self {
        Self {
            left: None,
            right_end: Some(right_end),
            right: None,
        }
    }
}

impl ShardBounds {
    pub fn left(&self) -> Option<&BlocksHeader> {
        self.left.as_ref()
    }

    pub fn right(&self) -> Option<&BlocksHeader> {
        self.right.as_ref()
    }

    pub fn left_replace(&mut self, left: BlocksHeader) -> Option<BlocksHeader> {
        self.left.replace(left)
    }

    pub fn right_replace(&mut self, right: BlocksHeader) -> Option<BlocksHeader> {
        self.right.replace(right)
    }

    pub fn right_end_replace(&mut self, right_end: Seqno) -> Option<Seqno> {
        self.right_end.replace(right_end)
    }

    pub fn right_next(&self) -> Option<Seqno> {
        let seqno = self.right_end?;

        match self.right {
            None => Some(seqno),
            Some(ref right) if right.id.seqno < seqno => Some(right.id.seqno + 1),
            _ => None,
        }
    }

    pub fn contains_seqno(&self, seqno: Seqno, not_available: bool) -> bool {
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
