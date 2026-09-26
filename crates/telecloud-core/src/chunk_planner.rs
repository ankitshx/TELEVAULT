use crate::models::{ChunkPlan, ChunkSlice};

pub struct ChunkPlanner;

impl ChunkPlanner {
    pub fn plan(file_size: u64, chunk_size: u64) -> ChunkPlan {
        let chunk_size = chunk_size.max(1024 * 1024); // Minimum 1 MB chunk safety floor

        if file_size == 0 {
            return ChunkPlan {
                file_size: 0,
                chunk_size,
                total_chunks: 1,
                slices: vec![ChunkSlice {
                    index: 0,
                    offset: 0,
                    size: 0,
                }],
            };
        }

        let total_chunks = ((file_size + chunk_size - 1) / chunk_size) as u32;
        let mut slices = Vec::with_capacity(total_chunks as usize);

        for index in 0..total_chunks {
            let offset = (index as u64) * chunk_size;
            let size = if index == total_chunks - 1 {
                file_size - offset
            } else {
                chunk_size
            };

            slices.push(ChunkSlice {
                index,
                offset,
                size,
            });
        }

        ChunkPlan {
            file_size,
            chunk_size,
            total_chunks,
            slices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_byte_file() {
        let plan = ChunkPlanner::plan(0, 100 * 1024 * 1024);
        assert_eq!(plan.total_chunks, 1);
        assert_eq!(plan.slices.len(), 1);
        assert_eq!(plan.slices[0].size, 0);
    }

    #[test]
    fn test_small_file_below_chunk_size() {
        // 10 KB file with 100 MB chunk size -> 1 chunk of 10 KB
        let plan = ChunkPlanner::plan(10 * 1024, 100 * 1024 * 1024);
        assert_eq!(plan.total_chunks, 1);
        assert_eq!(plan.slices[0].offset, 0);
        assert_eq!(plan.slices[0].size, 10 * 1024);
    }

    #[test]
    fn test_file_exactly_at_chunk_size() {
        let chunk_size = 50 * 1024 * 1024;
        let plan = ChunkPlanner::plan(chunk_size, chunk_size);
        assert_eq!(plan.total_chunks, 1);
        assert_eq!(plan.slices[0].size, chunk_size);
    }

    #[test]
    fn test_file_just_above_chunk_size() {
        let chunk_size = 50 * 1024 * 1024;
        let plan = ChunkPlanner::plan(chunk_size + 1, chunk_size);
        assert_eq!(plan.total_chunks, 2);
        assert_eq!(plan.slices[0].size, chunk_size);
        assert_eq!(plan.slices[1].size, 1);
        assert_eq!(plan.slices[1].offset, chunk_size);
    }

    #[test]
    fn test_multi_gigabyte_file() {
        // 5.8 GB file with 1 GB chunk size
        let file_size: u64 = 5_800_000_000;
        let chunk_size: u64 = 1_000_000_000;
        let plan = ChunkPlanner::plan(file_size, chunk_size);

        assert_eq!(plan.total_chunks, 6);
        assert_eq!(plan.slices.len(), 6);

        // First 5 chunks are 1 GB each
        for i in 0..5 {
            assert_eq!(plan.slices[i].size, chunk_size);
            assert_eq!(plan.slices[i].offset, (i as u64) * chunk_size);
        }
        // Last chunk is remaining 800 MB
        assert_eq!(plan.slices[5].size, 800_000_000);
        assert_eq!(plan.slices[5].offset, 5_000_000_000);

        // Sum of all slice sizes matches total file size exactly
        let sum: u64 = plan.slices.iter().map(|s| s.size).sum();
        assert_eq!(sum, file_size);
    }
}
