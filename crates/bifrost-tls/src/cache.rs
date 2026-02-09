use lru::LruCache;
use parking_lot::Mutex;
use rustls::sign::CertifiedKey;
use std::num::NonZeroUsize;
use std::sync::Arc;

const DEFAULT_CACHE_SIZE: usize = 1000;

#[derive(Debug)]
pub struct CertCache {
    cache: Mutex<LruCache<String, Arc<CertifiedKey>>>,
}

impl CertCache {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CACHE_SIZE)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let cap =
            NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap());
        Self {
            cache: Mutex::new(LruCache::new(cap)),
        }
    }

    pub fn get(&self, domain: &str) -> Option<Arc<CertifiedKey>> {
        self.cache.lock().get(domain).cloned()
    }

    pub fn insert(&self, domain: &str, cert: Arc<CertifiedKey>) {
        self.cache.lock().put(domain.to_string(), cert);
    }

    pub fn remove(&self, domain: &str) -> Option<Arc<CertifiedKey>> {
        self.cache.lock().pop(domain)
    }

    pub fn clear(&self) {
        self.cache.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cache.lock().cap().get()
    }
}

impl Default for CertCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::generate_root_ca;
    use crate::dynamic::DynamicCertGenerator;

    fn create_test_cert(domain: &str) -> Arc<CertifiedKey> {
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let generator = DynamicCertGenerator::new(ca);
        Arc::new(
            generator
                .generate_for_domain(domain)
                .expect("Failed to generate cert"),
        )
    }

    #[test]
    fn test_cache_new() {
        let cache = CertCache::new();
        assert_eq!(cache.capacity(), DEFAULT_CACHE_SIZE);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_with_capacity() {
        let cache = CertCache::with_capacity(500);
        assert_eq!(cache.capacity(), 500);
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = CertCache::new();
        let cert = create_test_cert("example.com");

        cache.insert("example.com", cert.clone());
        let retrieved = cache.get("example.com");

        assert!(retrieved.is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_miss() {
        let cache = CertCache::new();
        let result = cache.get("nonexistent.com");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_remove() {
        let cache = CertCache::new();
        let cert = create_test_cert("example.com");

        cache.insert("example.com", cert);
        assert_eq!(cache.len(), 1);

        let removed = cache.remove("example.com");
        assert!(removed.is_some());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_clear() {
        let cache = CertCache::new();
        cache.insert("example1.com", create_test_cert("example1.com"));
        cache.insert("example2.com", create_test_cert("example2.com"));

        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_lru_eviction() {
        let cache = CertCache::with_capacity(2);

        cache.insert("example1.com", create_test_cert("example1.com"));
        cache.insert("example2.com", create_test_cert("example2.com"));
        cache.insert("example3.com", create_test_cert("example3.com"));

        assert_eq!(cache.len(), 2);
        assert!(cache.get("example1.com").is_none());
        assert!(cache.get("example2.com").is_some());
        assert!(cache.get("example3.com").is_some());
    }
}
