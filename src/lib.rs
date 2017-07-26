#![feature(allocator_api)]
#![feature(alloc)]
#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

use core::mem;


trait Allocator {
    fn malloc<'a, T>(&mut self) -> Option<&'a mut T>;
    fn free<T>(&self, &mut T);
}


struct MemoryManager<'a> {
    tags: &'a mut [&'a mut BoundaryTag],
}


impl<'a> MemoryManager<'a> {
    fn new(tags: &'a mut [&'a mut BoundaryTag]) -> MemoryManager
    {
        debug_assert!(tags.len() != 0);

        MemoryManager {
            tags: tags,
        }
    }
}

impl<'a> Allocator for MemoryManager<'a> {
    fn malloc<'b, T>(&mut self) -> Option<&'b mut T>
    {
        let request_size = mem::size_of::<T>();
        let tag = self
            .tags
            .iter_mut()
            .find(|t| request_size < t.free_area_size);

        let tag =
            match tag {
                None => return None,
                Some(tag) => tag,
            };

        match BoundaryTag::divide(tag, request_size) {
            (_, None)           => None,
            (_, Some(free_tag)) => {
                free_tag.is_alloc = true;
                Some(unsafe { &mut *(free_tag.addr_free_area() as *mut T) })
            },
        }
    }

    fn free<T>(&self, _: &mut T)
    {
        // TODO
    }
}


#[repr(C)]
#[derive(Debug)]
struct BoundaryTag {
    is_alloc: bool,
    is_sentinel: bool,
    free_area_size: usize,
    prev_tag_addr: Option<usize>,
    next_tag_addr: Option<usize>,
}


impl<'a> BoundaryTag {
    fn addr(&self) -> usize
    {
        (self as *const _) as usize
    }

    fn addr_free_area(&self) -> usize
    {
        self.addr() + mem::size_of::<BoundaryTag>()
    }

    fn is_next_of(&self, tag: &'a mut BoundaryTag) -> bool
    {
        match BoundaryTag::next_tag_of(tag) {
            (_, Some(ref v)) if v.addr() == self.addr() => true,
            _  => false,
        }
    }

    fn is_prev_of(&self, tag: &'a mut BoundaryTag) -> bool
    {
        match BoundaryTag::prev_tag_of(tag) {
            (Some(ref v), _) if v.addr() == self.addr() => true,
            _  => false,
        }
    }

    unsafe fn cast_addr_tag_mut(addr: usize) -> &'a mut BoundaryTag
    {
        &mut *(addr as *mut BoundaryTag)
    }

    fn from_memory(addr: usize, size: usize) -> &'a mut BoundaryTag
    {
        let tag = unsafe { BoundaryTag::cast_addr_tag_mut(addr) };

        tag.is_alloc = false;
        tag.is_sentinel = true;
        tag.free_area_size = size - mem::size_of::<BoundaryTag>();
        tag.prev_tag_addr = None;
        tag.next_tag_addr = None;

        tag
    }

    fn divide(tag: &'a mut BoundaryTag, request_size: usize) -> (&'a mut BoundaryTag, Option<&'a mut BoundaryTag>)
    {
        let required_size = request_size + mem::size_of::<BoundaryTag>();
        if tag.free_area_size <= required_size {
            return (tag, None);
        }

        let free_area_size = tag.free_area_size;
        tag.free_area_size = tag.free_area_size - required_size;
        tag.is_sentinel = false;

        // Create new block at the tail of the tag.
        let new_tag_addr = tag.addr_free_area() + free_area_size - required_size;
        tag.next_tag_addr = Some(new_tag_addr);

        let new_tag = BoundaryTag::from_memory(new_tag_addr, required_size);
        new_tag.prev_tag_addr = Some(tag.addr());

        (tag, Some(new_tag))
    }

    // FIXME: This function will cause dangling pointer problems.
    fn merge(tag_x: &'a mut BoundaryTag, tag_y: &'a mut BoundaryTag) -> (&'a mut BoundaryTag)
    {
        // TODO: use Result type.
        let (tag_prev, tag_next) =
            match (tag_x.is_prev_of(tag_y), tag_x.is_next_of(tag_y)) {
                (true, false) => (tag_x, tag_y),
                (false, true) => (tag_y, tag_x),
                _ => panic!("FIXME: to handle the invalid cases"),
            };

        tag_prev.free_area_size += mem::size_of::<BoundaryTag>() + tag_next.free_area_size;
        tag_prev.is_sentinel = tag_next.is_sentinel;
        tag_prev.next_tag_addr = tag_next.next_tag_addr;

        tag_prev
    }

    fn next_tag_of(tag: &'a mut BoundaryTag) -> (&'a mut BoundaryTag, Option<&'a mut BoundaryTag>)
    {
        match tag.next_tag_addr {
            Some(addr) => (tag, Some(unsafe { BoundaryTag::cast_addr_tag_mut(addr) })),
            None       => (tag, None),
        }
    }

    fn prev_tag_of(tag: &'a mut BoundaryTag) -> (Option<&'a mut BoundaryTag>, &'a mut BoundaryTag)
    {
        match tag.prev_tag_addr {
            Some(addr) => (Some(unsafe { BoundaryTag::cast_addr_tag_mut(addr) }), tag),
            None       => (None, tag),
        }
    }
}


#[cfg(test)]
mod tests {
    use core::mem;

    use super::MemoryManager;
    use super::BoundaryTag;
    use super::Allocator;

    extern crate alloc;
    use self::alloc::allocator::Alloc;
    use self::alloc::allocator::Layout;
    use self::alloc::heap;

    fn allocate_memory() -> (usize, usize)
    {
        const SIZE: usize = 4096;
        let x = unsafe {
            let mut heap = heap::Heap;
            let l = Layout::from_size_align(SIZE, 1).unwrap();
            heap.alloc(l).unwrap()
        };

        let addr = (x as *const _) as usize;

        (addr, SIZE)
    }

    #[test]
    fn test_all()
    {
        let (addr, size) = allocate_memory();
        let tag1 = BoundaryTag::from_memory(addr, size);

        let mut tags = [tag1];
        let mut mman = MemoryManager::new(&mut tags);

        const SIZE: usize = 1024;
        let slice_opt = mman.malloc::<[u8; SIZE]>();
        assert_eq!(slice_opt.is_none(), false);
        let slice = slice_opt.unwrap();

        for i in &mut slice[..] {
            *i = 0xAF;
        }

        for i in &slice[..] {
            assert_eq!(*i, 0xAF);
        }
    }

    #[test]
    fn test_tag_size()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

        let request_size = size / 2;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>() * 2 - request_size);
        assert_eq!(new_tag.free_area_size, request_size);
        assert_eq!(size, tag.free_area_size + new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2);
    }

    #[test]
    #[should_panic]
    fn test_memory_manager_panic()
    {
        let slice: &mut [&mut BoundaryTag] = &mut [];
        let _ = MemoryManager::new(slice);
    }

    #[test]
    fn test_addr()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!(addr, tag.addr());
    }

    #[test]
    fn test_boundary_tag_from_memory()
    {
        let (addr, size) = allocate_memory();

        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!((tag as *const _) as usize, addr);
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, true);
    }

    #[test]
    fn test_divide()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

        let request_size = size;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        assert!(new_tag_opt.is_none());
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>() - request_size - mem::size_of::<BoundaryTag>());

        assert_eq!(new_tag.addr(), addr + mem::size_of::<BoundaryTag>() + tag.free_area_size);
        assert_eq!(new_tag.free_area_size, request_size);
        assert_eq!(new_tag.is_alloc, false);
        assert_eq!(new_tag.is_sentinel, true);

        assert_eq!(tag.free_area_size, size - (new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2));
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, false);

        assert_eq!(size, tag.free_area_size + new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2);
    }

    #[test]
    fn test_merge()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        let merged_tag = BoundaryTag::merge(tag, new_tag);
        assert_eq!(merged_tag.free_area_size, size - mem::size_of::<BoundaryTag>());
    }

    #[test]
    fn test_next_tag_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
        assert_eq!(next_tag_opt.is_none(), true);

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        assert_eq!(new_tag_opt.is_none(), false);

        let new_tag = new_tag_opt.unwrap();

        let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
        assert_eq!(next_tag_opt.is_none(), false);
        let next_tag = next_tag_opt.unwrap();

        assert_eq!(new_tag.addr(), next_tag.addr());
        assert_eq!(new_tag.free_area_size, next_tag.free_area_size);
        assert_eq!(new_tag.is_alloc, next_tag.is_alloc);
        assert_eq!(new_tag.is_sentinel, next_tag.is_sentinel);
        assert_eq!(tag.addr(), addr);

        let (next_tag, next_next_tag_opt) = BoundaryTag::next_tag_of(next_tag);
        assert_eq!(next_next_tag_opt.is_none(), true);

        assert_eq!(next_tag.free_area_size, request_size);
    }

    #[test]
    fn test_prev_tag_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);

        let (none, tag) = BoundaryTag::prev_tag_of(tag);
        assert_eq!(none.is_none(), true);

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        assert_eq!(new_tag_opt.is_none(), false);

        let new_tag = new_tag_opt.unwrap();
        let (prev_tag_opt, _) = BoundaryTag::prev_tag_of(new_tag);
        assert_eq!(prev_tag_opt.is_none(), false);

        let prev_tag = prev_tag_opt.unwrap();

        assert_eq!(prev_tag.addr(), addr);
        assert_eq!(prev_tag.addr(), tag.addr());
        assert_eq!(prev_tag.is_alloc, false);
        assert_eq!(prev_tag.is_sentinel, false);
        assert_eq!(prev_tag.free_area_size, size - (request_size + 2 * mem::size_of::<BoundaryTag>()));
    }

    #[test]
    fn test_addr_free_area()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!(tag.addr_free_area(), addr + mem::size_of::<BoundaryTag>());
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        assert_eq!(tag.addr(), addr);
        assert_eq!(tag.addr_free_area(), addr + mem::size_of::<BoundaryTag>());
        assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>() - request_size - mem::size_of::<BoundaryTag>());

        let new_tag = new_tag_opt.unwrap();
        assert_eq!(new_tag.addr_free_area(), new_tag.addr() + mem::size_of::<BoundaryTag>());
        assert_eq!(new_tag.free_area_size, request_size);
        assert_eq!(new_tag.addr(), tag.addr() + mem::size_of::<BoundaryTag>() + tag.free_area_size);
        assert_eq!(new_tag.addr_free_area(), tag.addr_free_area() + tag.free_area_size + mem::size_of::<BoundaryTag>());

        assert_eq!(tag.addr(), new_tag.addr() - tag.free_area_size - mem::size_of::<BoundaryTag>());
        assert_eq!(tag.addr(), new_tag.addr_free_area() - tag.free_area_size - mem::size_of::<BoundaryTag>() * 2);
    }

    #[test]
    fn test_is_next_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        assert_eq!(new_tag.is_next_of(tag), true);
        assert_eq!(tag.is_next_of(new_tag), false);
    }


    #[test]
    fn test_is_prev_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        assert_eq!(new_tag.is_prev_of(tag), false);
        assert_eq!(tag.is_prev_of(new_tag), true);
    }
}
