#!/usr/bin/env ruby
# frozen_string_literal: true

# Drop legacy Mullvad-only Swift source files from their build targets.
# These reference FFI symbols (`request_ephemeral_peer`,
# `EphemeralPeerParameters`, `start_udp2tcp_obfuscator_proxy`,
# `start_shadowsocks_obfuscator_proxy`, `start_quic_obfuscator_proxy`,
# `ProxyHandle`) that the Warren-side warren-ios FFI does not export —
# these are upstream Mullvad ephemeral-peer post-quantum + obfuscator
# proxy concepts that the Warren Quinn / wallet path doesn't use.
#
# Files stay on disk (preserves git history) but stop being compiled,
# mirroring the C.4.4.X pattern used for WgAdapter etc.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/remove-mullvad-legacy-sources.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

LEGACY_BASENAMES = %w[
  EphemeralPeerNegotiator.swift
  EphemeralPeerExchangeActor.swift
  EphemeralPeerReceiver.swift
  TunnelObfuscator.swift
  MullvadPostQuantum+Stubs.swift
].freeze

# NOTE : ProtocolObfuscator.swift was removed earlier but is referenced
# by PacketTunnelActor.swift via the `ProtocolObfuscation` namespace.
# Removing it cascades into PacketTunnelActor compile failures (49,
# 62). Keep it for now ; full Warren tunnel actor refactor is
# C.4.5 follow-up where PacketTunnelActor is replaced by a
# Warren-native actor that doesn't depend on Mullvad obfuscation.
#
# NOTE : TunnelObfuscator.swift (FFI-backed) IS removed but a no-op
# stub `TunnelObfuscator` + the `TunnelObfuscationProtocol` enum and
# `TunnelObfuscation` protocol live in `TunnelObfuscationTypes.swift`
# instead. ProtocolObfuscator can consume the stub since the
# protocol surface is identical.

# NOTE : WireGuardKey.swift is NOT removed. It provides the
# `WireGuard.PublicKey` namespace used by `WarrenTypes.RESTTypes.Device`
# + others. Removing it cascades into RESTTypes/Mullvad device-key
# flows that haven't been fully migrated to WarrenPubkey yet (deferred
# to a future C.4.5 rebrand step). The WireGuardKey FFI calls inside
# the file (mullvad_generate_private_key etc.) will fail compile when
# the file is in WarrenRustRuntime target — this is the same state as
# pre-this-script ; full fix requires gating the FFI methods behind
# warren-ios FFI stubs OR replacing the entire Device.pubkey contract.

removed = Hash.new { |h, k| h[k] = [] }

project.targets.each do |target|
  next unless target.respond_to?(:source_build_phase) && target.source_build_phase
  target.source_build_phase.files.dup.each do |build_file|
    next unless build_file.file_ref
    name = build_file.file_ref.respond_to?(:display_name) ? build_file.file_ref.display_name : nil
    next unless name && LEGACY_BASENAMES.include?(name)
    target.source_build_phase.files.delete(build_file)
    removed["sources:#{target.name}"] << name
  end
end

project.objects.dup.each do |obj|
  next unless obj.is_a?(Xcodeproj::Project::Object::PBXBuildFile)
  ref = obj.file_ref
  next unless ref && ref.respond_to?(:display_name)
  next unless LEGACY_BASENAMES.include?(ref.display_name)
  obj.remove_from_project
  removed["orphan"] << ref.display_name
end

total = removed.values.flatten.size
if total.zero?
  puts "No Mullvad-legacy source files to remove. Already clean."
else
  project.save
  puts "Removed #{total} entries:"
  removed.each { |group, items| puts "  [#{group}] #{items.size} : #{items.uniq.join(", ")}" }
end
