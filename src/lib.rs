#![no_std]
mod memory_region;

use core::mem;
use memory_region::MemoryRegion;


trait Allocator {
    fn malloc(usize) -> usize;
    fn free(usize);
}


struct MemoryManager<'a> {
    memory_regions: &'a [MemoryRegion],
    head: &'a BoundaryTag,
}


impl<'a> MemoryManager<'a> {
    fn new(mems: &'a [MemoryRegion]) -> MemoryManager
    {
        assert!(mems.len() != 0);

        // TODO: use all memory regions.
        let addr = mems[0].addr();
        let size = mems[0].size();

        MemoryManager {
            memory_regions: mems,
            head: BoundaryTag::from_memory(addr, size),
        }
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
    use super::MemoryRegion;
    use super::BoundaryTag;

    fn allocate_memory() -> (usize, usize)
    {
        const SIZE: usize = 4096;
        let ref mut x: [u8; SIZE] = unsafe { mem::zeroed() };

        assert_eq!(SIZE, mem::size_of_val(x));

        let addr = (x as *const _) as usize;

        (addr, SIZE)
    }

    #[test]
    fn test_all()
    {
        let (addr, size) = allocate_memory();

        let _ = MemoryRegion::new(addr, size);
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
        let slice: &[MemoryRegion] = &[];
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
