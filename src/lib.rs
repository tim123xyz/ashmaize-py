use std::sync::Arc;

use ashmaize::{Rom, RomGenerationType};
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use rand::RngCore;
use rayon::prelude::*;

#[pyclass]
struct PyRom {
    inner: Arc<Rom>,
}

#[pymethods]
impl PyRom {
    fn mine_batch(
        &self,
        preimage_static: &str,
        difficulty: u32,
        batch_size: u32,
    ) -> PyResult<String> {
        let rom = Arc::clone(&self.inner);
        let nonce_s = rand::rng().next_u64();
        if let Some(found) = (0..batch_size as u64).into_par_iter().find_map_any(|i| {
            let salt = build_random_salt(nonce_s + i, preimage_static);
            let hash = ashmaize::hash(salt.as_bytes(), &rom, 8, 256);
            if meets_difficulty(&hash, difficulty) {
                Some(salt)
            } else {
                None
            }
        }) {
            return Ok(found);
        }

        Ok(String::new())
    }

    /// Hash a single preimage with default parameters (8 loops, 256 instructions)
    fn hash(&self, preimage: &str) -> PyResult<String> {
        self.hash_with_params(preimage, 8, 256)
    }

    /// Hash with custom loop/instruction parameters
    fn hash_with_params(&self, preimage: &str, nb_loops: u32, nb_instrs: u32) -> PyResult<String> {
        let salt = preimage.as_bytes();
        let hash = ashmaize::hash(salt, &self.inner, nb_loops, nb_instrs);
        Ok(hex::encode(hash))
    }

    /// Hash multiple preimages in batch (FASTEST - all in Rust, minimal Python overhead)
    fn hash_batch(&self, preimages: Vec<String>) -> PyResult<Vec<String>> {
        self.hash_batch_with_params(preimages, 8, 256)
    }

    /// Hash batch with custom parameters
    fn hash_batch_with_params(
        &self,
        preimages: Vec<String>,
        nb_loops: u32,
        nb_instrs: u32,
    ) -> PyResult<Vec<String>> {
        let results: Vec<String> = preimages
            .into_par_iter()
            .map(|preimage| {
                let salt = preimage.as_bytes();
                let hash = ashmaize::hash(salt, &self.inner, nb_loops, nb_instrs);
                hex::encode(hash)
            })
            .collect();

        Ok(results)
    }
}

fn build_random_salt(nonce: u64, preimage_static: &str) -> String {
    hex::encode(nonce.to_ne_bytes()) + preimage_static
}

fn meets_difficulty(hash: &[u8; 64], difficulty_mask: u32) -> bool {
    let hash_prefix = u32::from_be_bytes([hash[0],hash[1],hash[2],hash[3]]);
    (hash_prefix | difficulty_mask) == difficulty_mask
}

/// Build a ROM from a key string (FullRandom generation)
#[pyfunction]
#[pyo3(signature = (key, size=1073741824))]
fn build_rom(py: Python, key: &str, size: usize) -> PyResult<PyRom> {
    let gen_type = RomGenerationType::FullRandom;

    let rom = py.detach(|| Rom::new(key.as_bytes(), gen_type, size));

    Ok(PyRom {
        inner: Arc::new(rom),
    })
}

/// Build a ROM from a key string using TwoStep generation (faster)
#[pyfunction]
#[pyo3(signature = (key, size=1073741824, pre_size=16777216, mixing_numbers=4))]
fn build_rom_twostep(
    py: Python,
    key: &str,
    size: usize,
    pre_size: usize,
    mixing_numbers: u32,
) -> PyResult<PyRom> {
    let gen_type = RomGenerationType::TwoStep {
        pre_size,
        mixing_numbers: mixing_numbers as usize,
    };

    let rom = py.detach(|| Rom::new(key.as_bytes(), gen_type, size));

    Ok(PyRom {
        inner: Arc::new(rom),
    })
}

#[pymodule]
fn ashmaize_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRom>()?;
    m.add_function(wrap_pyfunction!(build_rom, m)?)?;
    m.add_function(wrap_pyfunction!(build_rom_twostep, m)?)?;
    Ok(())
}
