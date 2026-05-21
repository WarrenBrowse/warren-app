// This file is generated automatically. To update it forcefully, run `cargo run -p warren-ios --target aarch64-apple-ios`.

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Used by Swift to instruct which access method kind it is trying to convert
 */
enum SwiftAccessMethodKind {
  KindDirect = 0,
  KindBridge,
  KindEncryptedDnsProxy,
  KindShadowsocks,
  KindSocks5Local,
  KindDomainFronting,
};
typedef uint8_t SwiftAccessMethodKind;

/**
 * Tunnel state enum surfaced via `warren_tunnel_status`. Variants
 * other than `Disconnected` are constructed by the warren-tunnel
 * dispatcher once the Quinn connection task is wired (C.4.1). On the
 * `not(tunnel)` feature path the enum has no constructor at all,
 * hence the `cfg_attr(expect)` suppression scoped to that path.
 */
typedef enum WarrenTunnelStateC {
  Disconnected = 0,
  Connecting = 1,
  Connected = 2,
  Reconnecting = 3,
  Failed = 4,
} WarrenTunnelStateC;

typedef struct ApiContext ApiContext;

typedef struct DomainFrontingConfigContext DomainFrontingConfigContext;

typedef struct Map Map;

typedef struct Option_WarrenTunnelEventCallback Option_WarrenTunnelEventCallback;

typedef struct Option_WarrenTunnelOutboundCallback Option_WarrenTunnelOutboundCallback;

typedef struct RequestCancelHandle RequestCancelHandle;

typedef struct RetryStrategy RetryStrategy;

typedef struct SwiftAccessMethodSettingsContext SwiftAccessMethodSettingsContext;

/**
 * Opaque handle representing an active Warren tunnel. Created by
 * [`warren_tunnel_start`] ; destroyed by [`warren_tunnel_stop`].
 * The Swift side treats this as an `OpaquePointer`.
 */
typedef struct WarrenTunnelHandle {
  uint8_t _private[0];
} WarrenTunnelHandle;

/**
 * Multi-hop entry relay configuration.
 */
typedef struct WarrenRelayConfigC {
  /**
   * 32-byte Ed25519 public key of the entry relay.
   */
  uint8_t pubkey[32];
  /**
   * Null-terminated UTF-8 "IP:port" of the entry relay.
   */
  const char *endpoint;
  /**
   * Null-terminated UTF-8 ISO 3166-1 alpha-2 country code.
   */
  const char *country_code;
} WarrenRelayConfigC;

/**
 * DAITA defensive shaping spec (cf. memory `warren_session_b_delivered`
 * M5.B.1 / `warren_daita_doctrine_v1`).
 */
typedef struct WarrenDaitaSpecC {
  /**
   * 32-byte Maybenot machine seed.
   */
  uint8_t machine_seed[32];
  /**
   * Padding budget in packets/sec.
   */
  uint32_t padding_pps;
} WarrenDaitaSpecC;

/**
 * Parameters passed from Swift to start a Warren tunnel. Mirrors the
 * public surface of `warren_tunnel::WarrenTunnelParameters`.
 *
 * String fields are null-terminated UTF-8 ; the Swift side allocates
 * and retains them for the duration of the `warren_tunnel_start`
 * call (Rust only borrows during marshalling).
 */
typedef struct WarrenTunnelParametersC {
  /**
   * 32-byte Ed25519 public key of the exit relay.
   */
  uint8_t exit_pubkey[32];
  /**
   * Null-terminated UTF-8 "IP:port" of the exit relay.
   */
  const char *exit_endpoint;
  /**
   * 32-byte Ed25519 signing seed derived from the user wallet
   * (see `warren_wallet_seed_from_mnemonic` + `derive_node_key`).
   */
  uint8_t wallet_signing_seed[32];
  /**
   * Optional multi-hop entry relay. Null when single-hop.
   */
  const struct WarrenRelayConfigC *multi_hop_relay;
  /**
   * Optional DAITA defensive shaping spec. Null when DAITA OFF.
   */
  const struct WarrenDaitaSpecC *daita_spec;
  /**
   * 1 enables NAT-PMP port forwarding through the tunnel ; 0 disables.
   */
  uint8_t nat_pmp_enabled;
  /**
   * Pointer to an array of null-terminated UTF-8 CIDRs to bypass
   * (see M4.H.G `--bypass-cidr`). Length given by `bypass_cidrs_count`.
   */
  const char *const *bypass_cidrs;
  /**
   * Number of entries in `bypass_cidrs`.
   */
  uint32_t bypass_cidrs_count;
} WarrenTunnelParametersC;

/**
 * Tunnel status snapshot.
 */
typedef struct WarrenTunnelStatusC {
  enum WarrenTunnelStateC state;
  uint64_t bytes_in;
  uint64_t bytes_out;
  /**
   * Seconds since the current connection was established. 0 when
   * `state != Connected`.
   */
  uint64_t connected_duration_seconds;
  /**
   * Cumulative failover count this session (cf. M5.B.2).
   */
  uint32_t failover_count;
} WarrenTunnelStatusC;

typedef struct SwiftApiContext {
  const struct ApiContext *_0;
} SwiftApiContext;

typedef struct SwiftAccessMethodSettingsWrapper {
  struct SwiftAccessMethodSettingsContext *_0;
} SwiftAccessMethodSettingsWrapper;

/**
 * Opaque wrapper around domain fronting configuration, created by
 * [`new_domain_fronting_config`] and consumed by the init functions.
 */
typedef struct SwiftDomainFrontingConfig {
  struct DomainFrontingConfigContext *_0;
} SwiftDomainFrontingConfig;

typedef struct SwiftShadowsocksLoaderWrapperContext {
  const void *shadowsocks_loader;
} SwiftShadowsocksLoaderWrapperContext;

typedef struct SwiftShadowsocksLoaderWrapper {
  struct SwiftShadowsocksLoaderWrapperContext _0;
} SwiftShadowsocksLoaderWrapper;

typedef struct SwiftData {
  void *ptr;
} SwiftData;

typedef struct SwiftCancelHandle {
  struct RequestCancelHandle *ptr;
} SwiftCancelHandle;

typedef struct SwiftRetryStrategy {
  struct RetryStrategy *_0;
} SwiftRetryStrategy;

typedef struct SwiftMullvadApiResponse {
  uint8_t *body;
  uintptr_t body_size;
  char *etag;
  uint16_t status_code;
  char *error_description;
  char *server_response_code;
  bool success;
} SwiftMullvadApiResponse;

typedef struct CompletionCookie {
  void *inner;
} CompletionCookie;

typedef struct SwiftServerMock {
  const void *server_ptr;
  const void *mock_ptr;
  uint16_t port;
} SwiftServerMock;

typedef struct ProblemReportMetadata {
  struct Map *inner;
} ProblemReportMetadata;

typedef struct SwiftProblemReportRequest {
  const char *address;
  const char *message;
  const char *log;
  struct ProblemReportMetadata metadata;
} SwiftProblemReportRequest;

/**
 * Callback function type for logging.
 * - `level`: The log level (1=Error, 2=Warn, 3=Info, 4=Debug, 5=Trace)
 * - `message`: Null-terminated UTF-8 string containing the log message
 */
typedef void (*LogCallback)(uint8_t level, const char *message);

/**
 * Starts a Warren tunnel with the given parameters. Returns an opaque
 * handle on success, or null on failure (invalid parameters, tunnel
 * feature disabled at build time, runtime allocation failure).
 *
 * `packet_fd` is the iOS `NEPacketTunnelFlow` file descriptor. iOS
 * does *not* expose the TUN fd directly through `NEPacketTunnelFlow` ;
 * pass `-1` and use [`warren_tunnel_inject_inbound_packet`] +
 * [`warren_tunnel_set_outbound_callback`] for the Swift bridge.
 *
 * # Safety
 * `parameters` must point to a valid `WarrenTunnelParametersC` (all
 * inner pointers valid for the duration of this call).
 */
struct WarrenTunnelHandle *warren_tunnel_start(const struct WarrenTunnelParametersC *parameters,
                                               int32_t _packet_fd);

/**
 * Stops the tunnel and releases all resources. Idempotent : safe to
 * call on a null handle (no-op).
 *
 * # Safety
 * `handle` must have been returned by [`warren_tunnel_start`] and must
 * not have been stopped already.
 */
void warren_tunnel_stop(struct WarrenTunnelHandle *handle);

/**
 * Triggers a tunnel reconnect (e.g. on Wi-Fi <-> cellular handover).
 * Uses `warren_backoff::Backoff::HANDSHAKE` (15s, cf. M4.H.G).
 *
 * Returns `0` on success, `-3` if the tunnel is not connected.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 */
int warren_tunnel_reconnect(struct WarrenTunnelHandle *handle);

/**
 * Reads the current tunnel status into `out_status`.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * `handle` may be null (status reports `Disconnected`). When non-null
 * it must be a valid pointer from [`warren_tunnel_start`].
 * `out_status` must point to a writable `WarrenTunnelStatusC`.
 */
int warren_tunnel_status(struct WarrenTunnelHandle *handle, struct WarrenTunnelStatusC *out_status);

/**
 * Registers a callback invoked on tunnel events (connected,
 * disconnected, reconnecting, failover, NAT-PMP events).
 *
 * Replaces any previously registered callback. Passing a null
 * callback clears the registration.
 *
 * Returns `0` on success.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 * `callback` (if non-null) must outlive the call to
 * [`warren_tunnel_stop`]. `context` is passed back unchanged ; lifetime
 * is the caller's responsibility.
 */
int warren_tunnel_set_event_callback(struct WarrenTunnelHandle *handle,
                                     struct Option_WarrenTunnelEventCallback callback,
                                     void *context);

/**
 * Registers a callback that ships outbound IP packets to Swift for
 * `NEPacketTunnelFlow.writePackets`. Replaces any previously
 * registered callback.
 *
 * Returns `0` on success, `-1` on null handle.
 *
 * # Safety
 * Same invariants as [`warren_tunnel_set_event_callback`].
 */
int warren_tunnel_set_outbound_callback(struct WarrenTunnelHandle *handle,
                                        struct Option_WarrenTunnelOutboundCallback callback,
                                        void *context);

/**
 * Pushes an inbound IP packet onto the tunnel uplink queue. Called
 * by Swift after each `NEPacketTunnelFlow.readPackets` completion.
 *
 * `data` is borrowed for the duration of this call ; Rust copies the
 * bytes before returning.
 *
 * Returns `0` on success, `-1` on null handle / null data / zero
 * length.
 *
 * # Safety
 * `handle` must be a valid pointer from [`warren_tunnel_start`].
 * `data` must point to at least `len` bytes of readable memory.
 */
int warren_tunnel_inject_inbound_packet(struct WarrenTunnelHandle *handle,
                                        const uint8_t *data,
                                        uintptr_t len);

/**
 * Generates a new BIP39 mnemonic with `word_count` words (12 or 24).
 *
 * Returns a heap-allocated C string. Caller MUST free via
 * `warren_wallet_free_mnemonic`. Returns null on error (invalid
 * word_count or RNG failure).
 *
 * # Safety
 * The returned pointer must be passed back to
 * `warren_wallet_free_mnemonic` exactly once. Reading the string after
 * freeing is undefined behaviour.
 */
char *warren_wallet_generate_mnemonic(uint32_t word_count);

/**
 * Frees a mnemonic string previously returned by
 * `warren_wallet_generate_mnemonic`. No-op on null.
 *
 * # Safety
 * `ptr` must have been returned by `warren_wallet_generate_mnemonic`
 * and must not have been freed already.
 */
void warren_wallet_free_mnemonic(char *ptr);

/**
 * Derives the Warren identity 32-byte seed from a BIP39 mnemonic.
 *
 * `mnemonic` : null-terminated UTF-8 BIP39 phrase (12 or 24 words).
 * `out_seed` : caller-provided buffer of at least 32 bytes ; written
 * on success.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * `mnemonic` must point to a valid null-terminated C string.
 * `out_seed` must point to a writable buffer of at least 32 bytes.
 */
int warren_wallet_seed_from_mnemonic(const char *mnemonic, uint8_t *out_seed);

/**
 * Derives the Ed25519 public key from a 32-byte seed.
 *
 * `seed` : caller-provided buffer of 32 bytes.
 * `out_pubkey` : caller-provided buffer of at least 32 bytes ; written
 * on success.
 *
 * Returns `0` on success, `-1` on invalid input.
 *
 * # Safety
 * Both `seed` and `out_pubkey` must point to writable buffers of at
 * least 32 bytes each.
 */
int warren_wallet_derive_pubkey(const uint8_t *seed, uint8_t *out_pubkey);

/**
 * Signs an arbitrary payload with the Ed25519 signing key derived
 * from `seed`.
 *
 * `seed` : 32-byte seed buffer.
 * `payload` : pointer to `payload_len` bytes.
 * `out_signature` : caller-provided buffer of at least 64 bytes ;
 * written on success.
 *
 * Returns `0` on success, `-1` on invalid input, `-2` on internal
 * signing error.
 *
 * # Safety
 * All pointers must be non-null and point to buffers of the documented
 * sizes.
 */
int warren_wallet_sign(const uint8_t *seed,
                       const uint8_t *payload,
                       uintptr_t payload_len,
                       uint8_t *out_signature);

/**
 * Called by Swift to set the available access methods
 */
void mullvad_api_update_access_methods(struct SwiftApiContext api_context,
                                       struct SwiftAccessMethodSettingsWrapper settings_wrapper);

/**
 * Called by Swift to update the currently used access methods
 *
 * # SAFETY
 * `access_method_id` must point to a null terminated string in a UUID format
 *
 */
void mullvad_api_use_access_method(struct SwiftApiContext api_context,
                                   const char *access_method_id);

/**
 * Called by Swift to trigger a fetching and caching of addresses
 *
 * # SAFETY
 *
 * this takes no arguments other than the API context. The API context
 * needs to be valid, and the function should not be called concurrently.
 */
void mullvad_api_update_address_cache(struct SwiftApiContext swift_api_context);

/**
 * Creates a [`SwiftDomainFrontingConfig`] that owns copies of the provided strings.
 *
 * # Safety
 *
 * Both `front` and `proxy_host` must be pointers to null-terminated strings.
 * The pointers only need to be valid for the duration of this call.
 */
struct SwiftDomainFrontingConfig new_domain_fronting_config(const char *front,
                                                            const char *proxy_host);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_new_tls_disabled(const char *host,
                                                         const char *address,
                                                         const char *encrypted_dns_domain,
                                                         struct SwiftDomainFrontingConfig domain_fronting,
                                                         struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                                         struct SwiftAccessMethodSettingsWrapper settings_provider,
                                                         void (*access_method_change_callback)(const void*,
                                                                                               const uint8_t*),
                                                         const void *access_method_change_context);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * access_method_change_callback is a function with the C calling convention which will be called
 * whenever the access method changes with a user-specified opaque pointer and a pointer to the bytes
 * of the access method's UUID. Note that this callback must remain valid for the lifetime of the
 * program.
 *
 * access_method_change_context is the pointer passed verbatim to the callback. It is not dereferenced
 * by the Rust code, but remains opaque.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_new(const char *host,
                                            const char *address,
                                            const char *encrypted_dns_domain,
                                            struct SwiftDomainFrontingConfig domain_fronting,
                                            struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                            struct SwiftAccessMethodSettingsWrapper settings_provider,
                                            void (*access_method_change_callback)(const void*,
                                                                                  const uint8_t*),
                                            const void *access_method_change_context);

/**
 * # Safety
 *
 * `host` must be a pointer to a null terminated string representing a hostname for Mullvad API host.
 * This hostname will be used for TLS validation but not used for domain name resolution.
 *
 * `address` must be a pointer to a null terminated string representing a socket address through which
 * the Mullvad API can be reached directly.
 *
 * If a context cannot be constructed this function will panic since the call site would not be able
 * to proceed in a meaningful way anyway.
 *
 * This function is safe.
 */
struct SwiftApiContext mullvad_api_init_inner(const char *host,
                                              const char *address,
                                              const char *encrypted_dns_domain,
                                              struct SwiftDomainFrontingConfig domain_fronting,
                                              bool disable_tls,
                                              struct SwiftShadowsocksLoaderWrapper bridge_provider,
                                              struct SwiftAccessMethodSettingsWrapper settings_provider,
                                              void (*access_method_change_callback)(const void*,
                                                                                    const uint8_t*),
                                              const void *access_method_change_context);

extern void swift_store_address_cache(const uint8_t *data, uint64_t data_size);

extern struct SwiftData swift_read_address_cache(void);

/**
 * Converts parameters into a `Box<AccessMethodSetting>` raw representation that
 * can be passed across the FFI boundary
 *
 * # SAFETY:
 * `unique_identifier` and `name` must point to valid memory regions and contain NULL terminators.
 * They are only valid for the duration of this call.
 *
 * `proxy_configuration` can be NULL, or must be a pointer gotten through
 * either the `convert_shadowsocks` or `convert_socks5` methods.
 */
void *convert_builtin_access_method_setting(const char *unique_identifier,
                                            const char *name,
                                            bool is_enabled,
                                            SwiftAccessMethodKind method_kind,
                                            const void *proxy_configuration);

/**
 * Creates a wrapper around a `Settings` object that can be safely sent across the FFI boundary.
 *
 * # SAFETY
 * `direct_method_raw`, `bridges_method_raw`, `encrypted_dns_method_raw` and
 * `domain_fronting_method_raw` must be raw pointers resulting from a call to
 * `convert_builtin_access_method_setting`.
 * `custom_methods_raw` is an array of pointers to instances of `AccessMethodSetting`
 */
struct SwiftAccessMethodSettingsWrapper init_access_method_settings_wrapper(const void *direct_method_raw,
                                                                            const void *bridges_method_raw,
                                                                            const void *encrypted_dns_method_raw,
                                                                            const void *domain_fronting_method_raw,
                                                                            const void *custom_methods_raw,
                                                                            uintptr_t custom_method_count);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_account(struct SwiftApiContext api_context,
                                                 void *completion_cookie,
                                                 struct SwiftRetryStrategy retry_strategy,
                                                 const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_create_account(struct SwiftApiContext api_context,
                                                    void *completion_cookie,
                                                    struct SwiftRetryStrategy retry_strategy);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_delete_account(struct SwiftApiContext api_context,
                                                    void *completion_cookie,
                                                    struct SwiftRetryStrategy retry_strategy,
                                                    const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_addresses(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_api_addrs_available(struct SwiftApiContext api_context,
                                                         void *completion_cookie,
                                                         struct SwiftRetryStrategy retry_strategy,
                                                         const void *access_method_setting);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `etag` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_relays(struct SwiftApiContext api_context,
                                                void *completion_cookie,
                                                struct SwiftRetryStrategy retry_strategy,
                                                const char *etag);

/**
 * Called by the Swift side to signal that a Mullvad API call should be cancelled.
 * After this call, the cancel token is no longer valid.
 *
 * # Safety
 *
 * `handle_ptr` must be pointing to a valid instance of `SwiftCancelHandle`.
 */
void mullvad_api_cancel_task(struct SwiftCancelHandle *handle_ptr);

/**
 * Called by the Swift side to signal that the Rust `SwiftCancelHandle` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `handle_ptr` must be pointing to a valid instance of `SwiftCancelHandle`.
 */
void mullvad_api_cancel_task_drop(struct SwiftCancelHandle *handle_ptr);

/**
 * Maps to `mullvadApiCompletionFinish` on Swift side to facilitate callback based completion flow when doing
 * network calls through Mullvad API on Rust side.
 *
 * # Safety
 *
 * `response` must be pointing to a valid instance of `SwiftMullvadApiResponse`.
 *
 * `completion_cookie` must be pointing to a valid instance of `CompletionCookie`. `CompletionCookie` is safe
 * because the pointer in `MullvadApiCompletion` is valid for the lifetime of the process where this type is
 * intended to be used.
 */
extern void mullvad_api_completion_finish(struct SwiftMullvadApiResponse response,
                                          struct CompletionCookie completion_cookie);

/**
 * Get device info via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_ios_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_ios_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_device(struct SwiftApiContext api_context,
                                                void *completion_cookie,
                                                struct SwiftRetryStrategy retry_strategy,
                                                const char *account_number,
                                                const char *identifier);

/**
 * Get devices info via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * the `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_get_devices(struct SwiftApiContext api_context,
                                                 void *completion_cookie,
                                                 struct SwiftRetryStrategy retry_strategy,
                                                 const char *account_number);

/**
 * create device via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * the `public_key` pointer must be a valid pointer to 32 unsigned bytes.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_create_device(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy,
                                                   const char *account_number,
                                                   const uint8_t *public_key);

/**
 * delete device via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_delete_device(struct SwiftApiContext api_context,
                                                   void *completion_cookie,
                                                   struct SwiftRetryStrategy retry_strategy,
                                                   const char *account_number,
                                                   const char *identifier);

/**
 * rotate device key via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the `account_number` must be a pointer to a null terminated string.
 * the `identifier` must be a pointer to a null terminated string.
 * the `public_key` pointer must be a valid pointer to 32 unsigned bytes.
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_rotate_device_key(struct SwiftApiContext api_context,
                                                       void *completion_cookie,
                                                       struct SwiftRetryStrategy retry_strategy,
                                                       const char *account_number,
                                                       const char *identifier,
                                                       const uint8_t *public_key);

/**
 * Converts parameters into a boxed `Shadowsocks` configuration that is safe
 * to send across the FFI boundary
 *
 * # SAFETY
 * `address` must be a pointer to at least `address_len` bytes.
 * `c_password` and `c_cipher` must be pointers to null terminated strings
 */
const void *new_shadowsocks_access_method_setting(const uint8_t *address,
                                                  uintptr_t address_len,
                                                  uint16_t port,
                                                  const char *c_password,
                                                  const char *c_cipher);

/**
 * Converts parameters into a boxed `Socks5Remote` configuration that is safe
 *
 * to send across the FFI boundary
 *
 * # SAFETY
 * `address` must be a pointer to at least `address_len` bytes.
 * `c_username` and `c_password` must be pointers to null terminated strings, or null
 */
const void *new_socks5_access_method_setting(const uint8_t *address,
                                             uintptr_t address_len,
                                             uint16_t port,
                                             const char *c_username,
                                             const char *c_password);

char *get_shadowsocks_chipers(void);

/**
 * Deallocates a CString returned by the Mullvad API client.
 *
 * # Safety
 *
 * `cstr_ptr` must be a pointer to a string allocated by another `mullvad_api` function.
 */
void mullvad_api_cstring_drop(char *cstr_ptr);

/**
 * # Safety
 *
 * `method` must be a pointer to a null terminated string representing the http method.
 *
 * `path` must be a pointer to a null terminated string representing the url path.
 *
 * `response_code` must be a usize representing the http response code.
 *
 * `response_body` must be a pointer to a null terminated string representing the body.
 *
 * This function is safe.
 */
struct SwiftServerMock mullvad_api_mock_get(const char *path,
                                            uintptr_t response_code,
                                            const uint8_t *response_body);

/**
 * # Safety
 *
 * `path` must be a pointer to a null terminated string representing the url path.
 *
 * `response_code` must be a usize representing the http response code.
 *
 * `match_body` must be a pointer to a null terminated json string representing the body the server expects.
 *
 * This function is safe.
 */
struct SwiftServerMock mullvad_api_mock_post(const char *path,
                                             uintptr_t response_code,
                                             const char *match_body);

/**
 * Called by the Swift side to signal that the Rust `SwiftServerMock` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `mock_ptr` must be pointing to a valid instance of `SwiftServerMock`. This function
 * is not safe to call multiple times with the same `SwiftServerMock`.
 */
void mullvad_api_mock_drop(struct SwiftServerMock mock_ptr);

/**
 * Send a problem report via the Mullvad API client.
 *
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * the string properties of `SwiftProblemReportRequest` must be pointers to a null terminated strings.
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_send_problem_report(struct SwiftApiContext api_context,
                                                         void *completion_cookie,
                                                         struct SwiftRetryStrategy retry_strategy,
                                                         struct SwiftProblemReportRequest request);

struct ProblemReportMetadata swift_problem_report_metadata_new(void);

/**
 * Add key and value pair to the `ProblemReportMetadata`
 *
 * # Safety
 *
 * `map.inner` must be non-null and point to a valid
 * - `key` must be a null-terminated UTF-8 string, containing LF-separated machines.
 * - `value` must be a valid pointer to some valid and aligned pointer-sized memory.
 */
bool swift_problem_report_metadata_add(struct ProblemReportMetadata map,
                                       const char *key,
                                       const char *value);

void swift_problem_report_metadata_free(struct ProblemReportMetadata map);

/**
 * Called by the Swift side to signal that the Rust `SwiftMullvadApiResponse` can be safely
 * dropped from memory.
 *
 * # Safety
 *
 * `response` must be pointing to a valid instance of `SwiftMullvadApiResponse`. This function
 * is not safe to call multiple times with the same `SwiftMullvadApiResponse`.
 */
void mullvad_response_drop(struct SwiftMullvadApiResponse response);

/**
 * Creates a retry strategy that never retries after failure.
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_never(void);

/**
 * Creates a retry strategy that retries `max_retries` times with a constant delay of `delay_sec`.
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_constant(uintptr_t max_retries,
                                                              uint64_t delay_sec);

/**
 * Creates a retry strategy that retries `max_retries` times with a exponantially increating delay.
 * The delay will never exceed `max_delay_sec`
 * The result needs to be consumed.
 */
struct SwiftRetryStrategy mullvad_api_retry_strategy_exponential(uintptr_t max_retries,
                                                                 uint64_t initial_sec,
                                                                 uint32_t factor,
                                                                 uint64_t max_delay_sec);

/**
 * Creates a `Shadowsocks` configuration.
 *
 * # SAFETY
 * `rawBridgeProvider` **must** be provided by a call to `init_swift_shadowsocks_loader_wrapper`
 * It is okay to persist it, and use it across multiple threads.
 */
extern const void *swift_get_shadowsocks_bridges(const void *rawBridgeProvider);

/**
 * Called by the Swift side in order to provide an object to rust that can create
 * Shadowsocks configurations
 *
 * # SAFETY
 * `shadowsocks_loader` **must be** pointing to a valid instance of a `SwiftShadowsocksBridgeProvider`
 * That instance's lifetime has to be equivalent to a `'static` lifetime in Rust
 * This function does not take ownership of `shadowsocks_loader`
 */
struct SwiftShadowsocksLoaderWrapper init_swift_shadowsocks_loader_wrapper(const void *shadowsocks_loader);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `account_number` must be a pointer to a null terminated string.
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_init_storekit_payment(struct SwiftApiContext api_context,
                                                           void *completion_cookie,
                                                           struct SwiftRetryStrategy retry_strategy,
                                                           const char *account_number);

/**
 * # Safety
 *
 * `api_context` must be pointing to a valid instance of `SwiftApiContext`. A `SwiftApiContext` is created
 * by calling `mullvad_api_init_new`.
 *
 * This function takes ownership of `completion_cookie`, which must be pointing to a valid instance of Swift
 * object `MullvadApiCompletion`. The pointer will be freed by calling `mullvad_api_completion_finish`
 * when completion finishes (in completion.finish).
 *
 * `retry_strategy` must have been created by a call to either of the following functions
 * `mullvad_api_retry_strategy_never`, `mullvad_api_retry_strategy_constant` or `mullvad_api_retry_strategy_exponential`
 *
 * `body` must be a pointer to a contiguous memory segment
 *
 * `body_size` must be the size of the body
 *
 * This function is not safe to call multiple times with the same `CompletionCookie`.
 */
struct SwiftCancelHandle mullvad_ios_check_storekit_payment(struct SwiftApiContext api_context,
                                                            void *completion_cookie,
                                                            struct SwiftRetryStrategy retry_strategy,
                                                            const uint8_t *body,
                                                            uintptr_t body_size);

extern uint8_t *swift_data_get_ptr(const struct SwiftData *data);

extern uintptr_t swift_data_get_len(const struct SwiftData *data);

extern void swift_data_drop(struct SwiftData *data);

/**
 * Initialize the Rust logger with a Swift callback.
 *
 * This function should be called once early in the application lifecycle,
 * before any Rust code that uses logging is invoked.
 *
 * # Safety
 * - `callback` must be a valid function pointer that remains valid for the lifetime of the program.
 * - This function is safe to call multiple times, but only the first call will have an effect.
 */
void init_rust_logging(LogCallback callback);
