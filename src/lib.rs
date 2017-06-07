#![no_std]
mod memory_region;

use core::mem;
use memory_region::MemoryRegion;


trait allocator {
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
}


impl<'a> BoundaryTag {
    fn from_memory(addr: usize, size: usize) -> &'a mut BoundaryTag
    {
        let tag = unsafe { &mut *(addr as *mut BoundaryTag) };

        tag.is_alloc = false;
        tag.is_sentinel = true;
        tag.size = size - mem::size_of::<BoundaryTag>();

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

        (tag, Some(new_tag))
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

        let request_size = size;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        assert!(new_tag_opt.is_none());

        let request_size = size / 4;
        let (tag, new_tag_opt) = BoundaryTag::divide_two_part(tag, request_size);
        let new_tag = new_tag_opt.unwrap();

        assert_eq!((new_tag as *const _) as usize, addr + tag.size);
        assert_eq!(new_tag.size, request_size);
        assert_eq!(new_tag.is_alloc, false);
        assert_eq!(new_tag.is_sentinel, true);

        assert_eq!(tag.size, size - (new_tag.size + mem::size_of::<BoundaryTag>() * 2));
        assert_eq!(tag.is_alloc, false);
        assert_eq!(tag.is_sentinel, false);

        assert_eq!(size, tag.size + new_tag.size + mem::size_of::<BoundaryTag>() * 2);
    }
}
