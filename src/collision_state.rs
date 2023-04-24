/// struct which maintains the collision state of the MSM points and and their buckets
/// specifically, it maintains a couple of linked lists as follows:
///   - unprocessed: list of points observed in collision
///   - processing: list of points added to current batch that will be processed
///   - free空闲: list of free slots(插槽) that could be used to store new points
/// 

#[derive(Debug)]
pub struct CollisionState {
    // entries are organized as multiple linked lists 条目被组织为多个链表
    //but stored as a single array for memory efficiency 但是为了内存效率而存储为单个数组
    entries: Vec<Entry>,
    collision_count: usize,
    max_collision_count: usize,
    free: Queue,
    processing: Queue,
    unprocessed: Queue
}

#[derive(Debug)]
struct Entry {
    pub data: u32,
    pub next: u32
}

#[derive(Clone, Copy)]

#[derive(Debug)]
struct Queue {
    pub head: u32,
    pub tail: u32
}

impl CollisionState {
    const NIL: u32 = u32::MAX;

    pub fn new(max_collision_count: usize) -> Self {
        // worst-case: all slices, except for the first slice, from the
        // unprocessed_list were added to the processing_list, and a new set of
        // collisions occur, causing the unprocessed_list to be filled again. To
        // accommodate both lists, we must allocate 2x max_collision_count
        let max_size = max_collision_count * 2;  

        //with_capacity 方法创建一个 Vec 实例时，实际上并没有分配任何元素的空间，只有在向 Vec 中添加元素时，才会开始分配和使用内存。
        //创建一个可能需要大量元素的 Vec 时，建议使用 with_capacity 方法来预先分配足够的内存，这样可以提高程序的性能。
        let mut _entries = Vec::with_capacity(max_size); 
        for i in 0..(max_size - 1) {
            _entries.push(Entry {
                data: 0,
                next: i as u32 + 1
            });
        }
        _entries.push(Entry { data: 0, next: Self::NIL });

        CollisionState {
            entries: _entries,
            collision_count: 0,
            max_collision_count,

            //free 队列用于跟踪哈希表中空闲的条目，可以用于添加新的元素或删除现有元素时重用空间。因此，在初始化时，free 队列应该包含整个 _entries 向量中的所有条目。
            free: Queue { head: 0, tail: max_size as u32 - 1 },
            //processing 队列用于跟踪正在处理的条目。在初始化时，哈希表还没有任何条目被处理，因此 processing 队列应该为空。
            processing: Queue { head: Self::NIL, tail: Self::NIL },
            unprocessed: Queue { head: Self::NIL, tail: Self::NIL },
        }
    }

    fn is_empty(queue: &Queue) -> bool {
        queue.head == Self::NIL
    }

    fn clear(queue: &mut Queue) {
        queue.head = Self::NIL;
        queue.tail = Self::NIL;
    }

    fn dequeue(queue: &mut Queue, entries: &[Entry]) -> u32 {
        if Self::is_empty(queue) {
            panic!("queue is empty");
        }

        let index = queue.head;
        if queue.head == queue.tail {
            Self::clear(queue);
        } else {
            queue.head = entries[index as usize].next;
        }
        return index;
    }

    fn enqueue(queue: &mut Queue, entries: &mut [Entry], entry_index: u32) {
        if Self::is_empty(queue) {
            queue.head = entry_index;
            queue.tail = entry_index;
        } else {
            entries[queue.tail as usize].next = entry_index;
            queue.tail = entry_index;
        }
        entries[entry_index as usize].next = Self::NIL;
    }

    pub fn get_entry_data(&mut self, entry_index: u32) -> u32 {
        return self.entries[entry_index as usize].data;
    }

    pub fn dequeue_unprocessed(&mut self) -> u32 {
        self.collision_count -= 1;
        return Self::dequeue(&mut self.unprocessed, &self.entries);
    }

    pub fn mark_entry_processing(&mut self, entry_index: u32) {
        Self::enqueue(&mut self.processing, &mut self.entries, entry_index);
    }

    pub fn mark_entry_unprocessed(&mut self, entry_index: u32) {
        self.collision_count += 1;
        Self::enqueue(&mut self.unprocessed, &mut self.entries, entry_index);
    }

    pub fn add_unprocessed(&mut self, data: u32) -> u32 {
        if Self::is_empty(&self.free) {
            panic!("No free slots available");
        }
        let index = Self::dequeue(&mut self.free, &self.entries);
        self.entries[index as usize].data = data;
        self.mark_entry_unprocessed(index);
        return index;
    }

    pub fn free_processing(&mut self) {
        if Self::is_empty(&self.processing) {
            return;
        }

        if Self::is_empty(&self.free) {
            self.free = self.processing;
        } else {
            self.entries[self.free.tail as usize].next = self.processing.head;
            self.free.tail = self.processing.tail;
        }
        Self::clear(&mut self.processing);
    }

    pub fn needs_processing(&self) -> bool {
        return !Self::is_empty(&self.unprocessed) ||
            !Self::is_empty(&self.processing);
    }

    pub fn reaches_max_collision_count(&self) -> bool {
        return self.collision_count >= self.max_collision_count;
    }

    pub fn get_unprocessed_tail(&self) -> u32 {
        return self.unprocessed.tail;
    }
}
