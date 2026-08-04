//! This module keeps track of the last known good API IP address and reads and stores it on disk.

use crate::{ApiEndpoint, DnsResolver};
use async_trait::async_trait;
use std::{fmt::Debug, io, net::SocketAddr, path::Path, sync::Arc};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Attempt to read without a path specified")]
    NoPath,

    #[error("Failed to open the address cache file")]
    Open(#[source] io::Error),

    #[error("Failed to read the address cache file")]
    Read(#[source] io::Error),

    #[error("Failed to parse the address cache file")]
    Parse,

    #[error("Failed to update the address cache file")]
    Write(#[source] io::Error),
}

/// a backing store for an AddressCache.

#[async_trait]
pub trait AddressCacheBacking: 'static + Sync + Send {
    async fn read(&self) -> Result<Vec<u8>, Error>;
    async fn write(&self, data: &[u8]) -> Result<(), Error>;
}

/// Read budget for the cache file. One `host:port` line, plus room for a
/// trailing newline and an IPv6 form, is well under this.
const MAX_CACHE_BYTES: u64 = 128;

#[derive(Clone, Debug)]
pub struct FileAddressCacheBacking {
    read_path: Option<Arc<Path>>,
    write_path: Option<Arc<Path>>,
}

#[async_trait]
impl AddressCacheBacking for FileAddressCacheBacking {
    async fn read(&self) -> Result<Vec<u8>, Error> {
        let Some(read_path) = self.read_path.as_ref() else {
            return Err(Error::NoPath);
        };
        let file = fs::File::open(read_path).await.map_err(Error::Open)?;
        let mut result = vec![];
        // `read_to_end`, never `read`: `read` fills the buffer it is given, and
        // an empty `Vec` has no room, so it reports 0 bytes and the cache
        // parses an empty string on every start-up. `take` because the content
        // is one `host:port` line: anything longer is not a cache file, and
        // reading it whole would be the file's author choosing our allocation.
        file.take(MAX_CACHE_BYTES)
            .read_to_end(&mut result)
            .await
            .map_err(Error::Read)?;
        Ok(result)
    }

    async fn write(&self, data: &[u8]) -> Result<(), Error> {
        let Some(write_path) = self.write_path.as_ref() else {
            return Err(Error::NoPath);
        };
        let mut file = mullvad_fs::AtomicFile::new(&**write_path)
            .await
            .map_err(Error::Open)?;
        file.write_all(data).await.map_err(Error::Write)?;
        file.finalize().await.map_err(Error::Write)
    }
}

/// A DNS resolver which resolves using `AddressCache`.
#[async_trait]
impl<T: AddressCacheBacking> DnsResolver for AddressCache<T> {
    async fn resolve(&self, host: String) -> Result<Vec<SocketAddr>, io::Error> {
        self.resolve_hostname(&host)
            .await
            .map(|addr| vec![addr])
            .ok_or(io::Error::other("host does not match API host"))
    }
}

#[derive(Clone, Debug)]
pub struct AddressCache<T> {
    hostname: String,
    inner: Arc<Mutex<AddressCacheInner>>,
    backing: Arc<T>,
}

/// Whether an address can actually be dialed, i.e. is neither the
/// "never resolved" sentinel ([`mullvad_api_constants::API_IP_DEFAULT`], the
/// unspecified address) nor portless.
///
/// The single definition of that rule: the address cache refuses to persist an
/// address that fails it, and every consumer (the firewall's allowed endpoint,
/// the daemon's API resolver) asks the same question of what it reads back.
#[must_use]
pub fn is_dialable(address: SocketAddr) -> bool {
    !address.ip().is_unspecified() && address.port() != 0
}

impl AddressCache<FileAddressCacheBacking> {
    /// A cache with no persisted address to read, for the runtimes that have no
    /// cache directory (API override, tests). Changes still go to `write_path`
    /// when one is given.
    pub fn new(
        endpoint: &ApiEndpoint,
        write_path: Option<Box<Path>>,
    ) -> AddressCache<FileAddressCacheBacking> {
        let backing = FileAddressCacheBacking {
            read_path: None,
            write_path: write_path.map(Arc::from),
        };
        AddressCache::new_with_address(endpoint, Arc::new(backing))
    }

    /// The start-up cache, reading from `read_path` and writing changes to
    /// `write_path`. See [`AddressCache::resolved_or_persisted`] for which of
    /// the two possible addresses wins.
    pub async fn for_startup(
        read_path: &Path,
        write_path: Option<Box<Path>>,
        endpoint: &ApiEndpoint,
    ) -> AddressCache<FileAddressCacheBacking> {
        let backing = FileAddressCacheBacking {
            read_path: Some(Arc::from(read_path)),
            write_path: write_path.map(Arc::from),
        };
        AddressCache::resolved_or_persisted(endpoint.host().to_owned(), Arc::new(backing), endpoint)
            .await
    }
}

impl<T: AddressCacheBacking> AddressCache<T> {
    /// Initialise cache using a hardcoded address and a Backing for writing to
    pub fn new_with_address(endpoint: &ApiEndpoint, backing: Arc<T>) -> Self {
        Self::new_inner(endpoint.address(), endpoint.host().to_owned(), backing)
    }

    pub async fn from_backing(hostname: String, backing: Arc<T>) -> Result<Self, Error> {
        let address = read_address_backing(backing.as_ref()).await?;
        Ok(Self::new_inner(address, hostname, backing))
    }

    pub async fn from_backing_or(
        hostname: String,
        backing: Arc<T>,
        endpoint: &ApiEndpoint,
    ) -> Self {
        Self::from_backing(hostname.clone(), Arc::clone(&backing))
            .await
            .unwrap_or(Self::new_inner(endpoint.address(), hostname, backing))
    }

    /// Build the cache a daemon starts from, and persist what it learned.
    ///
    /// The start-up lookup wins whenever it produced something dialable, so a
    /// daemon on a working network behaves exactly as it did before this cache
    /// was ever written. Persisting that answer is what the stored address is
    /// for: the NEXT start-up may happen on a host this daemon's own kill
    /// switch already fenced, where no name resolves, and without a stored
    /// address the API endpoint degrades to the unspecified sentinel, which
    /// permits nothing and leaves every recovery fetch on a resolver the
    /// blocking state drops.
    ///
    /// Nothing here can widen what the firewall admits: the stored value is a
    /// single `host:port` used exactly where the resolved one would have been,
    /// it is only ever consulted when the lookup produced nothing, and an
    /// absent, empty or corrupt file simply leaves the sentinel in place. A
    /// write failure is logged and ignored, never fatal: an unwritable cache
    /// dir must not stop the daemon from starting.
    ///
    /// The stored address is an address, never a name: the host name still
    /// travels on every request, so TLS certificate and hostname verification
    /// are untouched. An attacker able to write the (root-owned) cache file can
    /// therefore point the API connection at a machine of their choosing, but
    /// not authenticate as the API, and root-write on that file already implies
    /// control of the daemon binary itself.
    pub async fn resolved_or_persisted(
        hostname: String,
        backing: Arc<T>,
        endpoint: &ApiEndpoint,
    ) -> Self {
        // A missing, empty or corrupt file is simply "no persisted address":
        // it must never fail start-up and never widen anything. A read that
        // fails for another reason (an unreadable cache dir) is worth a line,
        // or an operator has nothing to diagnose it with.
        let persisted = match read_address_backing(backing.as_ref()).await {
            Ok(address) => Some(address),
            Err(Error::NoPath | Error::Parse) => None,
            Err(error) => {
                log::debug!("No persisted API address: {error}");
                None
            }
        };
        let resolved = endpoint.address();

        if !is_dialable(resolved) {
            return match persisted {
                Some(address) => {
                    log::info!("Using the persisted API address; this host resolves no name");
                    Self::new_inner(address, hostname, backing)
                }
                None => Self::new_inner(resolved, hostname, backing),
            };
        }

        let cache = Self::new_inner(resolved, hostname, backing);
        if persisted != Some(resolved)
            && let Err(error) = cache.save_to_backing(&resolved).await
        {
            log::warn!("Failed to persist the API address: {error}");
        }
        cache
    }

    fn new_inner(address: SocketAddr, hostname: String, backing: Arc<T>) -> Self {
        let cache = AddressCacheInner::from_address(address);
        log::debug!("Using API address: {}", cache.address);

        Self {
            inner: Arc::new(Mutex::new(cache)),
            hostname,
            backing,
        }
    }

    /// Returns the address if the hostname equals `API.host`. Otherwise, returns `None`.
    async fn resolve_hostname(&self, hostname: &str) -> Option<SocketAddr> {
        if hostname.eq_ignore_ascii_case(&self.hostname) {
            Some(self.get_address().await)
        } else {
            None
        }
    }

    /// Returns the currently selected address.
    pub async fn get_address(&self) -> SocketAddr {
        self.inner.lock().await.address
    }

    pub async fn set_address(&self, address: SocketAddr) -> Result<(), Error> {
        let mut inner = self.inner.lock().await;
        if address != inner.address {
            match self.save_to_backing(&address).await {
                Ok(()) | Err(Error::NoPath) => (),
                Err(err) => return Err(err),
            };
            inner.address = address;
        }
        Ok(())
    }

    async fn save_to_backing(&self, address: &SocketAddr) -> Result<(), Error> {
        let mut contents = address.to_string();
        contents += "\n";
        self.backing.write(contents.as_bytes()).await
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct AddressCacheInner {
    address: SocketAddr,
}

impl AddressCacheInner {
    fn from_address(address: SocketAddr) -> Self {
        Self { address }
    }
}

async fn read_address_backing(backing: &impl AddressCacheBacking) -> Result<SocketAddr, Error> {
    backing
        .read()
        .await
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| Error::Parse))
        .and_then(|a| a.trim().parse().map_err(|_| Error::Parse))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex as StdMutex;

    const HOST: &str = "api.example.test";

    fn endpoint(address: SocketAddr) -> ApiEndpoint {
        ApiEndpoint::new(HOST.to_owned(), address, false)
    }

    /// The "the lookup resolved nothing" endpoint: `ApiEndpoint::address`
    /// falls back to this exact sentinel when system DNS fails.
    fn unresolved_endpoint() -> ApiEndpoint {
        endpoint(SocketAddr::new(
            mullvad_api_constants::API_IP_DEFAULT,
            mullvad_api_constants::API_PORT_DEFAULT,
        ))
    }

    /// Backing that holds its bytes in memory, so the start-up rule is tested
    /// without a filesystem. `None` models a file that is not there.
    #[derive(Default)]
    struct MemoryBacking {
        content: StdMutex<Option<Vec<u8>>>,
        writes: StdMutex<usize>,
    }

    impl MemoryBacking {
        fn holding(content: &str) -> Self {
            Self {
                content: StdMutex::new(Some(content.as_bytes().to_vec())),
                writes: StdMutex::new(0),
            }
        }

        fn stored(&self) -> Option<String> {
            self.content
                .lock()
                .unwrap()
                .as_ref()
                .map(|bytes| String::from_utf8(bytes.clone()).unwrap())
        }

        fn writes(&self) -> usize {
            *self.writes.lock().unwrap()
        }
    }

    #[async_trait]
    impl AddressCacheBacking for MemoryBacking {
        async fn read(&self) -> Result<Vec<u8>, Error> {
            self.content.lock().unwrap().clone().ok_or(Error::NoPath)
        }

        async fn write(&self, data: &[u8]) -> Result<(), Error> {
            *self.content.lock().unwrap() = Some(data.to_vec());
            *self.writes.lock().unwrap() += 1;
            Ok(())
        }
    }

    /// Backing whose writes always fail, modelling an unwritable cache dir.
    struct UnwritableBacking;

    #[async_trait]
    impl AddressCacheBacking for UnwritableBacking {
        async fn read(&self) -> Result<Vec<u8>, Error> {
            Err(Error::NoPath)
        }

        async fn write(&self, _data: &[u8]) -> Result<(), Error> {
            Err(Error::Write(io::Error::other("read-only cache dir")))
        }
    }

    fn scratch_file(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mullvad-api-address-cache-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[tokio::test]
    async fn a_written_address_reads_back_from_the_file() {
        // The whole persistence mechanism rests on this round trip: a file
        // that cannot be read back is the same as no file at all.
        let path = scratch_file("roundtrip");
        let backing = FileAddressCacheBacking {
            read_path: Some(Arc::from(path.as_path())),
            write_path: Some(Arc::from(path.as_path())),
        };

        backing.write(b"203.0.113.7:443\n").await.unwrap();

        let address = read_address_backing(&backing).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(address, "203.0.113.7:443".parse::<SocketAddr>().unwrap());
    }

    #[tokio::test]
    async fn an_oversized_cache_file_is_read_no_further_than_its_budget() {
        // The file is root-owned, so this is not an attack path; it is the
        // difference between a bounded read and letting the file decide how
        // much the daemon allocates at start-up.
        let path = scratch_file("oversized");
        let backing = FileAddressCacheBacking {
            read_path: Some(Arc::from(path.as_path())),
            write_path: Some(Arc::from(path.as_path())),
        };
        std::fs::write(&path, vec![b'9'; 1024 * 1024]).unwrap();

        let read = backing.read().await.unwrap();
        let parsed = read_address_backing(&backing).await;
        let _ = std::fs::remove_file(&path);

        assert_eq!(read.len() as u64, MAX_CACHE_BYTES);
        assert!(
            parsed.is_err(),
            "a truncated read must fail to parse, never yield half an address"
        );
    }

    #[tokio::test]
    async fn a_resolved_address_wins_and_is_persisted_for_the_next_start_up() {
        let backing = Arc::new(MemoryBacking::default());
        let resolved: SocketAddr = "203.0.113.7:443".parse().unwrap();

        let cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &endpoint(resolved),
        )
        .await;

        assert_eq!(cache.get_address().await, resolved);
        assert_eq!(backing.stored().as_deref(), Some("203.0.113.7:443\n"));
    }

    #[tokio::test]
    async fn a_resolved_address_wins_over_a_stale_persisted_one() {
        let backing = Arc::new(MemoryBacking::holding("198.51.100.1:443\n"));
        let resolved: SocketAddr = "203.0.113.7:443".parse().unwrap();

        let cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &endpoint(resolved),
        )
        .await;

        assert_eq!(cache.get_address().await, resolved);
        assert_eq!(backing.stored().as_deref(), Some("203.0.113.7:443\n"));
    }

    #[tokio::test]
    async fn an_unchanged_address_is_not_rewritten() {
        // The file is written on every start-up otherwise, for nothing.
        let backing = Arc::new(MemoryBacking::holding("203.0.113.7:443\n"));

        let _cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &endpoint("203.0.113.7:443".parse().unwrap()),
        )
        .await;

        assert_eq!(backing.writes(), 0);
    }

    #[tokio::test]
    async fn a_start_up_that_resolves_nothing_falls_back_to_the_persisted_address() {
        // The point of persisting: this is a daemon restarted onto a host its
        // own kill switch already fenced.
        let backing = Arc::new(MemoryBacking::holding("203.0.113.7:443\n"));

        let cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &unresolved_endpoint(),
        )
        .await;

        assert_eq!(
            cache.get_address().await,
            "203.0.113.7:443".parse::<SocketAddr>().unwrap()
        );
    }

    #[tokio::test]
    async fn a_corrupt_persisted_address_leaves_the_sentinel_rather_than_widening_anything() {
        let backing = Arc::new(MemoryBacking::holding("not an address at all"));

        let cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &unresolved_endpoint(),
        )
        .await;

        assert!(
            !is_dialable(cache.get_address().await),
            "a corrupt cache must degrade to the unspecified sentinel, never to \
             an address nothing verified"
        );
    }

    #[tokio::test]
    async fn the_unresolved_sentinel_is_never_persisted() {
        // Storing it would make the next start-up read back an address that
        // permits nothing, and mask a lookup that could have succeeded.
        let backing = Arc::new(MemoryBacking::default());

        let _cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::clone(&backing),
            &unresolved_endpoint(),
        )
        .await;

        assert_eq!(backing.stored(), None);
    }

    #[tokio::test]
    async fn an_unwritable_cache_never_fails_start_up() {
        let resolved: SocketAddr = "203.0.113.7:443".parse().unwrap();

        let cache = AddressCache::resolved_or_persisted(
            HOST.to_owned(),
            Arc::new(UnwritableBacking),
            &endpoint(resolved),
        )
        .await;

        assert_eq!(cache.get_address().await, resolved);
    }

    #[test]
    fn only_a_dialable_address_is_worth_persisting() {
        assert!(is_dialable("203.0.113.7:443".parse().unwrap()));
        assert!(!is_dialable("0.0.0.0:443".parse().unwrap()));
        assert!(!is_dialable("203.0.113.7:0".parse().unwrap()));
        assert!(!is_dialable("[::]:443".parse().unwrap()));
    }
}
