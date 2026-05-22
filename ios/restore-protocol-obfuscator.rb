#!/usr/bin/env ruby
# frozen_string_literal: true

# Restore ProtocolObfuscator.swift to PacketTunnelCore target after
# over-aggressive removal in `remove-mullvad-legacy-sources.rb`. The
# file is referenced by `PacketTunnelActor.swift` via the
# `ProtocolObfuscation` namespace — removing it cascades into compile
# errors. Full Warren-native PacketTunnelActor replacement is a C.4.5
# follow-up.
#
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

file_ref = project.files.find do |f|
  f.real_path.to_s.end_with?("PacketTunnelCore/Actor/ProtocolObfuscator.swift")
end

unless file_ref
  warn "ProtocolObfuscator.swift file_ref not found"
  exit 1
end

target = project.targets.find { |t| t.name == "PacketTunnelCore" }
if target && target.respond_to?(:source_build_phase) && target.source_build_phase
  if target.source_build_phase.files_references.include?(file_ref)
    puts "ProtocolObfuscator.swift already wired into PacketTunnelCore. OK."
  else
    target.add_file_references([file_ref])
    project.save
    puts "Restored ProtocolObfuscator.swift to PacketTunnelCore"
  end
end
