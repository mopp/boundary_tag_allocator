#![feature(allocator_api)]
#![feature(alloc)]
#![feature(unique)]
#![no_std]

#[cfg(test)]
#[macro_use]
extern crate std;

use core::mem;
use core::ptr::Unique;


trait Allocator {
    fn malloc<'a, T>(&mut self) -> Option<&'a mut T>;
    fn free<T>(&self, &mut T);
}


struct MemoryManager<'a> {
    tags: &'a mut [Unique<BoundaryTag>],
}


impl<'a> MemoryManager<'a> {
    fn new(tags: &'a mut [Unique<BoundaryTag>]) -> MemoryManager
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
            .find(|t| request_size < unsafe {t.as_ref()}.free_area_size);

        let tag =
            match tag {
                None => return None,
                Some(tag) => tag,
            };

        match BoundaryTag::divide(*tag, request_size) {
            (_, None)           => None,
            (_, Some(mut free_tag)) => {
                let t = unsafe {free_tag.as_mut()};
                t.is_alloc = true;
                Some(unsafe { &mut *(t.addr_free_area() as *mut T) })
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

    fn is_next_of(&self, tag: &Unique<BoundaryTag>) -> bool
    {
        match BoundaryTag::next_tag_of(tag) {
            Some(ref next_tag) if next_tag.addr() == self.addr() => true,
            _ => false,
        }
    }

    fn is_prev_of(&self, tag: &Unique<BoundaryTag>) -> bool
    {
        match BoundaryTag::prev_tag_of(tag) {
            Some(ref prev_tag) if prev_tag.addr() == self.addr() => true,
            _ => false,
        }
    }

    unsafe fn new_from_addr(addr: usize) -> Unique<BoundaryTag>
    {
        Unique::new(addr as *mut BoundaryTag)
    }

    fn from_memory(addr: usize, size: usize) -> Unique<BoundaryTag>
    {
        let mut tag = unsafe { BoundaryTag::new_from_addr(addr) };
        {
            let mut tag_mut        = unsafe {tag.as_mut()};
            tag_mut.is_alloc       = false;
            tag_mut.is_sentinel    = true;
            tag_mut.free_area_size = size - mem::size_of::<BoundaryTag>();
            tag_mut.prev_tag_addr  = None;
            tag_mut.next_tag_addr  = None;
        }

        tag
    }

    fn divide(mut tag: Unique<BoundaryTag>, request_size: usize) -> (Unique<BoundaryTag>, Option<Unique<BoundaryTag>>)
    {
        let new_tag =
        {
            let mut tag_mut = unsafe {tag.as_mut()};
            let required_size = request_size + mem::size_of::<BoundaryTag>();
            if tag_mut.free_area_size <= required_size {
                None
            } else {
                let free_area_size     = tag_mut.free_area_size;
                tag_mut.free_area_size = tag_mut.free_area_size - required_size;
                tag_mut.is_sentinel    = false;

                // Create new block at the tail of the tag.
                let new_tag_addr = tag_mut.addr_free_area() + free_area_size - required_size;
                tag_mut.next_tag_addr = Some(new_tag_addr);

                let mut new_tag = BoundaryTag::from_memory(new_tag_addr, required_size);
                unsafe {new_tag.as_mut()}.prev_tag_addr = Some(tag_mut.addr());
                Some(new_tag)
            }
        };

        (tag, new_tag)
    }

    fn merge(tag_x: Unique<BoundaryTag>, tag_y: Unique<BoundaryTag>) -> Unique<BoundaryTag>
    {
        // TODO: use Result type.
        let (mut tag_prev, tag_next) =
            unsafe {
                match (tag_x.as_ref().is_prev_of(&tag_y), tag_x.as_ref().is_next_of(&tag_y)) {
                    (true, false) => (tag_x, tag_y),
                    (false, true) => (tag_y, tag_x),
                    _ => panic!("FIXME: to handle the invalid cases"),
                }
            };

        {
            let tag_next_ref = unsafe { tag_next.as_ref() };
            let tag_prev_mut = unsafe { tag_prev.as_mut() };
            tag_prev_mut.free_area_size += mem::size_of::<BoundaryTag>() + tag_next_ref.free_area_size;
            tag_prev_mut.is_sentinel     = tag_next_ref.is_sentinel;
            tag_prev_mut.next_tag_addr   = tag_next_ref.next_tag_addr;
        }

        tag_prev
    }

    fn next_tag_of(tag: &'a Unique<BoundaryTag>) -> Option<&'a mut BoundaryTag>
    {
        let tag_ref = unsafe{ tag.as_ref() };
        match tag_ref.next_tag_addr {
            Some(addr) => Some(unsafe { &mut *(addr as *mut BoundaryTag) }),
            None       => None
        }
    }

    fn prev_tag_of(tag: &'a Unique<BoundaryTag>) -> Option<&'a mut BoundaryTag>
    {
        let tag_ref = unsafe{ tag.as_ref() };
        match tag_ref.prev_tag_addr {
            Some(addr) => Some(unsafe { &mut *(addr as *mut BoundaryTag) }),
            None       => None
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

    // #[test]
    // fn test_all()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag1 = BoundaryTag::from_memory(addr, size);

    //     let mut tags = [tag1];
    //     let mut mman = MemoryManager::new(&mut tags);

    //     const SIZE: usize = 1024;
    //     let slice_opt = mman.malloc::<[u8; SIZE]>();
    //     assert_eq!(slice_opt.is_none(), false);
    //     let slice = slice_opt.unwrap();

    //     for i in &mut slice[..] {
    //         *i = 0xAF;
    //     }

    //     for i in &slice[..] {
    //         assert_eq!(*i, 0xAF);
    //     }
    // }

    // #[test]
    // fn test_tag_size()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag = BoundaryTag::from_memory(addr, size);
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

    //     let request_size = size / 2;
    //     let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
    //     let new_tag = new_tag_opt.unwrap();
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>() * 2 - request_size);
    //     assert_eq!(new_tag.free_area_size, request_size);
    //     assert_eq!(size, tag.free_area_size + new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2);
    // }

    // #[test]
    // #[should_panic]
    // fn test_memory_manager_panic()
    // {
    //     let slice: &mut [&mut BoundaryTag] = &mut [];
    //     let _ = MemoryManager::new(slice);
    // }

    // #[test]
    // fn test_addr()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag = BoundaryTag::from_memory(addr, size);
    //     assert_eq!(addr, tag.addr());
    // }

    // #[test]
    // fn test_boundary_tag_from_memory()
    // {
    //     let (addr, size) = allocate_memory();

    //     let tag = BoundaryTag::from_memory(addr, size);
    //     assert_eq!((tag as *const _) as usize, addr);
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());
    //     assert_eq!(tag.is_alloc, false);
    //     assert_eq!(tag.is_sentinel, true);
    // }

    // #[test]
    // fn test_divide()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag = BoundaryTag::from_memory(addr, size);
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

    //     let request_size = size;
    //     let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
    //     assert!(new_tag_opt.is_none());
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>());

    //     let request_size = size / 4;
    //     let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
    //     let new_tag = new_tag_opt.unwrap();
    //     assert_eq!(tag.free_area_size, size - mem::size_of::<BoundaryTag>() - request_size - mem::size_of::<BoundaryTag>());

    //     assert_eq!(new_tag.addr(), addr + mem::size_of::<BoundaryTag>() + tag.free_area_size);
    //     assert_eq!(new_tag.free_area_size, request_size);
    //     assert_eq!(new_tag.is_alloc, false);
    //     assert_eq!(new_tag.is_sentinel, true);

    //     assert_eq!(tag.free_area_size, size - (new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2));
    //     assert_eq!(tag.is_alloc, false);
    //     assert_eq!(tag.is_sentinel, false);

    //     assert_eq!(size, tag.free_area_size + new_tag.free_area_size + mem::size_of::<BoundaryTag>() * 2);
    // }

    // #[test]
    // fn test_merge()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag = BoundaryTag::from_memory(addr, size);
    //     let request_size = size / 4;
    //     let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
    //     let new_tag = new_tag_opt.unwrap();

    //     let merged_tag = BoundaryTag::merge(tag, new_tag);
    //     assert_eq!(merged_tag.free_area_size, size - mem::size_of::<BoundaryTag>());
    // }

    // #[test]
    // fn test_next_tag_of()
    // {
    //     let (addr, size) = allocate_memory();
    //     let tag = BoundaryTag::from_memory(addr, size);
    //     let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
    //     assert_eq!(next_tag_opt.is_none(), true);

    //     let request_size = size / 4;
    //     let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
    //     assert_eq!(new_tag_opt.is_none(), false);

    //     let new_tag = new_tag_opt.unwrap();

    //     let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
    //     assert_eq!(next_tag_opt.is_none(), false);
    //     let next_tag = next_tag_opt.unwrap();

    //     assert_eq!(new_tag.addr(), next_tag.addr());
    //     assert_eq!(new_tag.free_area_size, next_tag.free_area_size);
    //     assert_eq!(new_tag.is_alloc, next_tag.is_alloc);
    //     assert_eq!(new_tag.is_sentinel, next_tag.is_sentinel);
    //     assert_eq!(tag.addr(), addr);

    //     let (next_tag, next_next_tag_opt) = BoundaryTag::next_tag_of(next_tag);
    //     assert_eq!(next_next_tag_opt.is_none(), true);

    //     assert_eq!(next_tag.free_area_size, request_size);
    // }

    #[test]
    fn test_prev_tag_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);

        let none = BoundaryTag::prev_tag_of(&tag);
        assert_eq!(none.is_none(), true);

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        assert_eq!(new_tag_opt.is_none(), false);

        let new_tag = new_tag_opt.unwrap();
        let prev_tag_opt = BoundaryTag::prev_tag_of(&new_tag);
        assert_eq!(prev_tag_opt.is_none(), false);

        let prev_tag = prev_tag_opt.unwrap();

        assert_eq!(prev_tag.addr(), addr);
        assert_eq!(prev_tag.addr(), unsafe {tag.as_ref()}.addr());
        assert_eq!(prev_tag.is_alloc, false);
        assert_eq!(prev_tag.is_sentinel, false);
        assert_eq!(prev_tag.free_area_size, size - (request_size + 2 * mem::size_of::<BoundaryTag>()));
    }

    #[test]
    fn test_addr_free_area()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        {
            let tag_ref = unsafe { tag.as_ref() };
            assert_eq!(tag_ref.addr_free_area(), addr + mem::size_of::<BoundaryTag>());
            assert_eq!(tag_ref.free_area_size, size - mem::size_of::<BoundaryTag>());
        }

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        {
            let tag_ref = unsafe { tag.as_ref() };
            assert_eq!(tag_ref.addr(), addr);
            assert_eq!(tag_ref.addr_free_area(), addr + mem::size_of::<BoundaryTag>());
            assert_eq!(tag_ref.free_area_size, size - mem::size_of::<BoundaryTag>() - request_size - mem::size_of::<BoundaryTag>());
        }

        let new_tag     = new_tag_opt.unwrap();
        let tag_ref     = unsafe { tag.as_ref() };
        let new_tag_ref = unsafe { new_tag.as_ref() };
        assert_eq!(new_tag_ref.addr_free_area(), new_tag_ref.addr() + mem::size_of::<BoundaryTag>());
        assert_eq!(new_tag_ref.free_area_size, request_size);
        assert_eq!(new_tag_ref.addr(), tag_ref.addr() + mem::size_of::<BoundaryTag>() + tag_ref.free_area_size);
        assert_eq!(new_tag_ref.addr_free_area(), tag_ref.addr_free_area() + tag_ref.free_area_size + mem::size_of::<BoundaryTag>());

        assert_eq!(tag_ref.addr(), new_tag_ref.addr() - tag_ref.free_area_size - mem::size_of::<BoundaryTag>());
        assert_eq!(tag_ref.addr(), new_tag_ref.addr_free_area() - tag_ref.free_area_size - mem::size_of::<BoundaryTag>() * 2);
    }

    #[test]
    fn test_is_next_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        unsafe {
            assert_eq!(new_tag.as_ref().is_next_of(&tag), true);
            assert_eq!(tag.as_ref().is_next_of(&new_tag), false);
        }
    }


    #[test]
    fn test_is_prev_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        unsafe {
            assert_eq!(new_tag.as_ref().is_prev_of(&tag), false);
            assert_eq!(tag.as_ref().is_prev_of(&new_tag), true);
        }
    }
}
