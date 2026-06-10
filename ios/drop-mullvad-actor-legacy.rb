#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.5 Warren-native PacketTunnelActor : drop the Mullvad-era
# `PacketTunnelActor` + its 10 satellite files (PacketTunnelActor+*) +
# `ProtocolObfuscator` from the build. They depend on EphemeralPeer /
# TunnelObfuscation Mullvad legacy that has no counterpart in Warren.
#
# Warren's tunnel actor is `WarrenQuinnActor` (cf.
# `PacketTunnelCore/Actor/WarrenQuinnActor.swift`) wired through
# `WarrenQuinnTunnelImplementation`. The DEBUG-only `GotaTunActor`
# survives as an alt local-dev UI smoke path (it implements
# `PacketTunnelActorProtocol` directly without the Mullvad deps).
#
# Files dropped from PacketTunnelCore target:
#   PacketTunnelActor.swift
#   PacketTunnelActor+ConnectionMonitoring.swift
#   PacketTunnelActor+ErrorState.swift
#   PacketTunnelActor+Extensions.swift
#   PacketTunnelActor+KeyPolicy.swift
#   PacketTunnelActor+PostQuantum.swift
#   PacketTunnelActor+Public.swift
#   PacketTunnelActor+SleepCycle.swift
#   PacketTunnelActorReducer.swift
#   PacketTunnelActorCommand.swift
#   ProtocolObfuscator.swift
#
# Files dropped from PacketTunnelCoreTests target:
#   PacketTunnelActorTests.swift
#   PacketTunnelActorReducerTests.swift
#   ProtocolObfuscatorTests.swift
#   PacketTunnelActor+Mocks.swift
#   the EphemeralPeer/PostQuantum cluster (exchangers + their tests +
#   EphemeralPeerExchangeActorStub) : dead in the PQ-free Warren path,
#   only the warren-ios FFI Quinn transport remains.
#
# `PacketTunnelActorProtocol.swift` is KEPT (its ephemeral-negotiation
# methods stay vestigial in the contract), both `WarrenQuinnActor` and
# `GotaTunActor` conform to it. `PacketTunnelActorStub.swift` is KEPT
# (AppMessageHandlerTests still uses it).
#
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

DROP_FROM_CORE = %w[
  PacketTunnelActor.swift
  PacketTunnelActor+ConnectionMonitoring.swift
  PacketTunnelActor+ErrorState.swift
  PacketTunnelActor+Extensions.swift
  PacketTunnelActor+KeyPolicy.swift
  PacketTunnelActor+PostQuantum.swift
  PacketTunnelActor+Public.swift
  PacketTunnelActor+SleepCycle.swift
  PacketTunnelActorReducer.swift
  PacketTunnelActorCommand.swift
  ProtocolObfuscator.swift
  EventChannel.swift
].freeze

DROP_FROM_CORE_TESTS = %w[
  PacketTunnelActorTests.swift
  PacketTunnelActorReducerTests.swift
  ProtocolObfuscatorTests.swift
  EventChannelTests.swift
  TunnelObfuscationStub.swift
  ProtocolObfuscationStub.swift
  PacketTunnelActor+Mocks.swift
  EphemeralPeerExchangeActorStub.swift
  EphemeralPeerExchangingPipeline.swift
  EphemeralPeerExchangingPipelineTests.swift
  MultiHopEphemeralPeerExchanger.swift
  MultiHopEphemeralPeerExchangerTests.swift
  SingleHopEphemeralPeerExchanger.swift
  SingleHopEphemeralPeerExchangerTests.swift
].freeze

DROP_FROM_RUST_RUNTIME = %w[
  TunnelObfuscationTypes.swift
].freeze

DROP_FROM_RUST_RUNTIME_TESTS = %w[
  TunnelObfuscationTests.swift
  EphemeralPeerExchangeActorTests.swift
].freeze

def drop_from_target(project, target_name, filenames)
  target = project.targets.find { |t| t.name == target_name }
  return [] unless target&.respond_to?(:source_build_phase) && target.source_build_phase
  dropped = []
  filenames.each do |fname|
    file_ref = project.files.find { |f| f.real_path.to_s.end_with?("/#{fname}") }
    next unless file_ref
    build_files = target.source_build_phase.files.select { |bf| bf.file_ref == file_ref }
    next if build_files.empty?
    build_files.each { |bf| target.source_build_phase.remove_build_file(bf) }
    dropped << "#{fname} from #{target_name}"
  end
  dropped
end

dropped = []
dropped += drop_from_target(project, "PacketTunnelCore", DROP_FROM_CORE)
dropped += drop_from_target(project, "PacketTunnelCoreTests", DROP_FROM_CORE_TESTS)
dropped += drop_from_target(project, "WarrenRustRuntime", DROP_FROM_RUST_RUNTIME)
dropped += drop_from_target(project, "WarrenRustRuntimeTests", DROP_FROM_RUST_RUNTIME_TESTS)

if dropped.empty?
  puts "All files already absent from their targets. No changes."
else
  project.save
  puts "Dropped #{dropped.size} build phase entries:"
  dropped.each { |d| puts "  - #{d}" }
end
