#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.4 follow-up: drop the WireGuard/PostQuantum/ICMP legacy files
# from the PacketTunnel + PacketTunnelCore + WarrenRustRuntime build
# targets. Warren tunnels via Quinn (`warren-tunnel`) with a static
# Ed25519 wallet identity ; the post-quantum WG handshake + ICMP
# liveness pinger have no counterpart in the Warren path.
#
# Idempotent : files already absent from a target are silently
# skipped.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# Map of (path suffix matcher) -> targets to unwire from. The matcher
# is `String#end_with?` against `file_ref.real_path.to_s`.
DROP_MAP = {
  "PacketTunnel/PostQuantum/EphemeralPeerExchangingPipeline.swift" => %w[PacketTunnel],
  "PacketTunnel/PostQuantum/MultiHopEphemeralPeerExchanger.swift" => %w[PacketTunnel],
  "PacketTunnel/PostQuantum/SingleHopEphemeralPeerExchanger.swift" => %w[PacketTunnel],
  "PacketTunnelCore/Pinger/TunnelPinger.swift" => %w[PacketTunnel PacketTunnelCore],
  "PacketTunnel/PacketTunnelProvider/WireGuardGoTunnelImplementation.swift" => %w[PacketTunnel],
  "WarrenRustRuntime/EphemeralPeerExchangeActor.swift" => %w[WarrenRustRuntime],
}.freeze

dropped = []

DROP_MAP.each do |suffix, target_names|
  file_ref = project.files.find { |f| f.real_path.to_s.end_with?(suffix) }
  unless file_ref
    warn "drop: file_ref not found for #{suffix}"
    next
  end
  target_names.each do |target_name|
    target = project.targets.find { |t| t.name == target_name }
    next unless target && target.respond_to?(:source_build_phase) && target.source_build_phase
    build_files = target.source_build_phase.files.select { |bf| bf.file_ref == file_ref }
    next if build_files.empty?
    build_files.each do |bf|
      target.source_build_phase.remove_build_file(bf)
    end
    dropped << "#{suffix} from #{target_name}"
  end
end

if dropped.empty?
  puts "All files already absent from their targets. No changes."
else
  project.save
  puts "Dropped #{dropped.size} build phase entries:"
  dropped.each { |d| puts "  - #{d}" }
end
