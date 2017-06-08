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
    size: usize,
    prev_tag_size: usize,
}


impl<'a> BoundaryTag {
    fn addr(&self) -> usize
    {
        (self as *const _) as usize
    }

    fn from_memory(addr: usize, size: usize) -> &'a mut BoundaryTag
    {
        let tag = unsafe { &mut *(addr as *mut BoundaryTag) };

        tag.is_alloc = false;
        tag.is_sentinel = true;
        tag.size = size - mem::size_of::<BoundaryTag>();
        tag.prev_tag_size = 0;

        tag
    }

    fn divide_two_part(tag: &'a mut BoundaryTag, request_size: usize) -> (&'a mut BoundaryTag, Option<&'a mut BoundaryTag>)
    {
        let required_size = request_size + mem::size_of::<BoundaryTag>();
        if tag.size <= required_size {
            return (tag, None);
        }

        // Create new block at the tail of the tag.
        let current_tag_size = tag.size - required_size;
        tag.size = current_tag_size;
        tag.is_sentinel = false;

        let new_tag_addr = (tag as *const _) as usize + tag.size;
        let new_tag = BoundaryTag::from_memory(new_tag_addr, required_size);
        new_tag.prev_tag_size = tag.size;

        (tag, Some(new_tag))
    }

    fn next_tag_of(tag: &'a mut BoundaryTag) -> (&'a mut BoundaryTag, Option<&'a mut BoundaryTag>)
    {
        if tag.is_sentinel {
            return (tag, None);
        }

        let addr = tag.addr() + tag.size;
        let next_tag = unsafe { &mut *(addr as *mut BoundaryTag) };
        (tag, Some(next_tag))
    }

    fn prev_tag_of(tag: &'a mut BoundaryTag) -> (Option<&'a mut BoundaryTag>, &'a mut BoundaryTag)
    {
        if tag.prev_tag_size == 0 {
            return (None, tag)
        }

        let addr = tag.addr() - tag.prev_tag_size;
        let prev_tag = unsafe { &mut *(addr as *mut BoundaryTag) };
        (Some(prev_tag), tag)
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
        assert_eq!(tag.size, size - mem::size_of::<BoundaryTag>());
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, true);
    }

    #[test]
    fn test_divide_two_part()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        assert_eq!(tag.size, size - mem::size_of::<BoundaryTag>());

        let request_size = size;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        assert!(new_tag_opt.is_none());

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        let new_tag = new_tag_opt.unwrap();
        assert_eq!(tag.size, size - mem::size_of::<BoundaryTag>() - request_size - mem::size_of::<BoundaryTag>());

        assert_eq!((new_tag as *const _) as usize, addr + tag.size);
        assert_eq!(new_tag.size, request_size);
        assert_eq!(new_tag.is_alloc, false);
        assert_eq!(new_tag.is_sentinel, true);

        assert_eq!(tag.size, size - (new_tag.size + mem::size_of::<BoundaryTag>() * 2));
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, false);

        assert_eq!(size, tag.size + new_tag.size + mem::size_of::<BoundaryTag>() * 2);
    }

    #[test]
    fn test_next_tag_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);
        let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
        assert_eq!(next_tag_opt.is_none(), true);

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        assert_eq!(new_tag_opt.is_none(), false);

        let new_tag = new_tag_opt.unwrap();

        let (tag, next_tag_opt) = BoundaryTag::next_tag_of(tag);
        assert_eq!(next_tag_opt.is_none(), false);
        let next_tag = next_tag_opt.unwrap();

        assert_eq!(new_tag.addr(), next_tag.addr());
        assert_eq!(new_tag.size, next_tag.size);
        assert_eq!(new_tag.is_alloc, next_tag.is_alloc);
        assert_eq!(new_tag.is_sentinel, next_tag.is_sentinel);
        assert_eq!(tag.addr(), addr);

        let (next_tag, next_next_tag_opt) = BoundaryTag::next_tag_of(next_tag);
        assert_eq!(next_next_tag_opt.is_none(), true);

        assert_eq!(next_tag.size, request_size);
    }

    #[test]
    fn test_prev_tag_of()
    {
        let (addr, size) = allocate_memory();
        let tag = BoundaryTag::from_memory(addr, size);

        let (none, tag) = BoundaryTag::prev_tag_of(tag);
        assert_eq!(none.is_none(), true);

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        assert_eq!(new_tag_opt.is_none(), false);

        let new_tag = new_tag_opt.unwrap();
        let (prev_tag_opt, _) = BoundaryTag::prev_tag_of(new_tag);
        assert_eq!(prev_tag_opt.is_none(), false);

        let prev_tag = prev_tag_opt.unwrap();

        assert_eq!(prev_tag.addr(), addr);
        assert_eq!(prev_tag.addr(), tag.addr());
        assert_eq!(prev_tag.is_alloc, false);
        assert_eq!(prev_tag.is_sentinel, false);
        assert_eq!(prev_tag.size, size - (request_size + 2 * mem::size_of::<BoundaryTag>()));
    }
}
