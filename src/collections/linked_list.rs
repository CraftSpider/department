use crate::base::{MultiItemStorage, Storage};
use std::alloc::Layout;

type NodeRef<T, S> = <S as Storage>::Handle<Node<T, S>>;

struct Node<T, S: Storage> {
    next: Option<NodeRef<T, S>>,
    prev: Option<NodeRef<T, S>>,
    value: T,
}

pub struct LinkedList<T, S: Storage + MultiItemStorage> {
    nodes: Option<(NodeRef<T, S>, NodeRef<T, S>)>,
    len: usize,
    storage: S,
}

impl<T, S: Storage + MultiItemStorage> LinkedList<T, S> {
    /// # Safety
    ///
    /// Node ref passed must not have any mutable refs to it currently live
    unsafe fn node_val(&self, node: NodeRef<T, S>) -> &T {
        unsafe { &self.storage.get(node).as_ref().value }
    }

    /// # Safety
    ///
    /// Node ref passed must not have any other refs to it currently live
    unsafe fn node_val_mut(&self, node: NodeRef<T, S>) -> &mut T {
        unsafe { &mut self.storage.get(node).as_mut().value }
    }

    fn init_list(&mut self, value: T) -> &mut T {
        assert!(self.nodes.is_none());
        println!("Handle Layout: {:?}", Layout::new::<NodeRef<T, S>>());
        println!("Item Layout: {:?}", Layout::new::<T>());
        println!("Node Layout: {:?}", Layout::new::<Node<T, S>>());
        let first_node = self
            .storage
            .create(Node {
                next: None,
                prev: None,
                value,
            })
            .unwrap_or_else(|(err, _)| panic!("Storage Error: {}", err));
        let first = self.nodes.insert((first_node, first_node)).0;
        unsafe { self.node_val_mut(first) }
    }

    fn fix_refs(
        &mut self,
        prev: Option<NodeRef<T, S>>,
        new: NodeRef<T, S>,
        next: Option<NodeRef<T, S>>,
    ) {
        let (first, last) = self.nodes.as_mut().unwrap();

        let last_ref = prev.map(|handle| (handle, unsafe { self.storage.get(handle).as_mut() }));
        let next_ref = next.map(|handle| (handle, unsafe { self.storage.get(handle).as_mut() }));

        last_ref.map(|(prev, prev_ref)| {
            prev_ref.next = Some(new);

            if prev == *last {
                *last = new;
            }
        });

        next_ref.map(|(next, next_ref)| {
            next_ref.prev = Some(new);

            if next == *first {
                *first = new;
            }
        });
    }

    fn insert_node_after(&mut self, node: NodeRef<T, S>, value: T) -> &mut T {
        let node_ref: &mut Node<T, S> = unsafe { self.storage.get(node).as_mut() };

        let new_next = node_ref.next;

        let new_node = self
            .storage
            .create(Node {
                next: new_next,
                prev: Some(node),
                value,
            })
            .unwrap_or_else(|_| panic!());

        self.fix_refs(Some(node), new_node, new_next);

        unsafe { self.node_val_mut(new_node) }
    }

    fn first_node(&self) -> Option<NodeRef<T, S>> {
        Some(self.nodes?.0)
    }

    fn last_node(&self) -> Option<NodeRef<T, S>> {
        Some(self.nodes?.1)
    }

    pub fn new_in(storage: S) -> LinkedList<T, S> {
        LinkedList {
            nodes: None,
            len: 0,
            storage,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, value: T) -> &mut T {
        self.len += 1;
        match self.last_node() {
            Some(node) => self.insert_node_after(node, value),
            None => self.init_list(value),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let mut cur = self.first_node()?;
        for _ in 0..index {
            cur = unsafe { self.storage.get(cur).as_ref() }.next?;
        }
        Some(unsafe { self.node_val(cur) })
    }
}

impl<T, S: Storage + MultiItemStorage + Default> LinkedList<T, S> {
    pub fn new() -> LinkedList<T, S> {
        LinkedList::new_in(S::default())
    }
}

impl<T, S: Storage + MultiItemStorage> Drop for LinkedList<T, S> {
    fn drop(&mut self) {
        let (first, last) = match self.nodes {
            Some(nodes) => nodes,
            None => return,
        };

        let mut cur = first;

        loop {
            let next = unsafe { self.storage.get(cur).as_ref() }.next;
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
