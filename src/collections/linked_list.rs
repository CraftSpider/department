use crate::base::{MultiItemStorage, Storage};

type NodeRef<T, S> = <S as Storage>::Handle<Node<T, S>>;

struct Node<T, S: Storage> {
    next: Option<NodeRef<T, S>>,
    prev: Option<NodeRef<T, S>>,
    value: T,
}

/// A linked-list built on a storage. Generally [`Vec`][super::Vec] is more efficient, but this
/// may be useful in some niche use-cases.
pub struct LinkedList<T, S: Storage + MultiItemStorage> {
    nodes: Option<(NodeRef<T, S>, NodeRef<T, S>)>,
    len: usize,
    storage: S,
}

impl<T, S: Storage + MultiItemStorage> LinkedList<T, S> {
    /// # Safety
    ///
    /// Node ref passed must not have any mutable refs to it currently live, and be valid
    unsafe fn node_val(&self, node: NodeRef<T, S>) -> &T {
        // SAFETY: Our safety conditions require this is valid
        unsafe { &self.storage.get(node).as_ref().value }
    }

    /// # Safety
    ///
    /// Node ref passed must not have any other refs to it currently live, and be valid
    unsafe fn node_val_mut(&mut self, node: NodeRef<T, S>) -> &mut T {
        // SAFETY: Our safety conditions require this is valid
        unsafe { &mut self.storage.get(node).as_mut().value }
    }

    fn init_list(&mut self, value: T) -> &mut T {
        assert!(self.nodes.is_none());
        let first_node = self
            .storage
            .create(Node {
                next: None,
                prev: None,
                value,
            })
            .unwrap_or_else(|(err, _)| panic!("Storage Error: {}", err));
        let first = self.nodes.insert((first_node, first_node)).0;
        // SAFETY: We uniquely borrow self, and we just allocated this handle
        unsafe { self.node_val_mut(first) }
    }

    fn fix_refs(
        &mut self,
        prev: Option<NodeRef<T, S>>,
        new: NodeRef<T, S>,
        next: Option<NodeRef<T, S>>,
    ) {
        let (first, last) = self.nodes.as_mut().unwrap();

        let last_ref = prev.map(|handle| {
            // SAFETY: We uniquely borrow self, no one else should have refs right now
            (handle, unsafe { self.storage.get(handle).as_mut() })
        });
        let next_ref = next.map(|handle| {
            // SAFETY: We uniquely borrow self, no one else should have refs right now
            (handle, unsafe { self.storage.get(handle).as_mut() })
        });

        if let Some((prev, prev_ref)) = last_ref {
            prev_ref.next = Some(new);

            if prev == *last {
                *last = new;
            }
        };

        if let Some((next, next_ref)) = next_ref {
            next_ref.prev = Some(new);

            if next == *first {
                *first = new;
            }
        };
    }

    fn insert_node_after(&mut self, node: NodeRef<T, S>, value: T) -> &mut T {
        // SAFETY: We uniquely borrow self, no one else should have refs right now
        let node_ref: &mut Node<T, S> = unsafe { self.storage.get(node).as_mut() };

        let new_next = node_ref.next;

        let new_node = self
            .storage
            .create(Node {
                next: new_next,
                prev: Some(node),
                value,
            })
            .unwrap_or_else(|(err, _)| panic!("Storage Error: {}", err));

        self.fix_refs(Some(node), new_node, new_next);

        // SAFETY: We uniquely borrow self, and we just allocated this node
        unsafe { self.node_val_mut(new_node) }
    }

    fn first_node(&self) -> Option<NodeRef<T, S>> {
        Some(self.nodes?.0)
    }

    fn last_node(&self) -> Option<NodeRef<T, S>> {
        Some(self.nodes?.1)
    }

    /// Create a new linked-list using the provided storage
    pub fn new_in(storage: S) -> LinkedList<T, S> {
        LinkedList {
            nodes: None,
            len: 0,
            storage,
        }
    }

    /// Get the length of this list
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check whether this list is empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Add a new item to the end of this list
    pub fn push(&mut self, value: T) -> &mut T {
        self.len += 1;
        match self.last_node() {
            Some(node) => self.insert_node_after(node, value),
            None => self.init_list(value),
        }
    }

    /// Get an item from this list by index, returning None if the index is invalid
    pub fn get(&self, index: usize) -> Option<&T> {
        let mut cur = self.first_node()?;
        for _ in 0..index {
            // SAFETY: Nodes in our list should all have valid pointers
            //         we immutably borrow self, so node should be valid to borrow
            cur = unsafe { self.storage.get(cur).as_ref() }.next?;
        }
        // SAFETY: We immutable borrow self, and we got this node from our internal list
        Some(unsafe { self.node_val(cur) })
    }
}

impl<T, S: Storage + MultiItemStorage + Default> LinkedList<T, S> {
    /// Create a new, empty, [`LinkedList`].
    pub fn new() -> LinkedList<T, S> {
        LinkedList::new_in(S::default())
    }
}

impl<T, S: Storage + MultiItemStorage + Default> Default for LinkedList<T, S> {
    fn default() -> Self {
        LinkedList::new()
    }
}

impl<T, S: Storage + MultiItemStorage> Drop for LinkedList<T, S> {
    fn drop(&mut self) {
        let (first, _) = match self.nodes {
            Some(nodes) => nodes,
            None => return,
        };

        let mut cur = first;

        loop {
            // SAFETY: We have unique access and are in drop, no one else should be observing
            //         nodes, and all internal node refs should be valid
            let next = unsafe { self.storage.get(cur).as_ref() }.next;
            // SAFETY: All nodes should be valid and initialized, we're last observer
            unsafe { self.storage.drop(cur) };
            match next {
                Some(next) => cur = next,
                None => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LinkedList;
    use crate::alloc::GlobalAlloc;

    #[test]
    fn test_push() {
        let mut list = LinkedList::<i32, GlobalAlloc>::new();
        assert_eq!(list.len(), 0);
        list.push(1);
        assert_eq!(list.len(), 1);
        list.push(2);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_get() {
        let mut list = LinkedList::<i32, GlobalAlloc>::new();

        assert_eq!(list.get(0), None);

        list.push(1);
        list.push(2);

        assert_eq!(list.get(0), Some(&1));
        assert_eq!(list.get(1), Some(&2));
        assert_eq!(list.get(2), None);
    }
}
