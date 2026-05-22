#!/usr/bin/env ruby
# frozen_string_literal: true

# Restore TunnelObfuscator.swift to its target. Was removed by
# `remove-mullvad-legacy-sources.rb` but the `TunnelObfuscation`
# namespace it provides is consumed by ProtocolObfuscator.swift +
# PacketTunnelActor.swift. Full Warren-native obfuscation refactor is
# C.4.5 follow-up.
#
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

file_ref = project.files.find do |f|
  f.real_path.to_s.end_with?("WarrenRustRuntime/TunnelObfuscator.swift")
end

unless file_ref
  warn "TunnelObfuscator.swift file_ref not found"
  exit 1
end

target = project.targets.find { |t| t.name == "WarrenRustRuntime" }
if target && target.respond_to?(:source_build_phase) && target.source_build_phase
  if target.source_build_phase.files_references.include?(file_ref)
    puts "TunnelObfuscator.swift already wired into WarrenRustRuntime. OK."
  else
    target.add_file_references([file_ref])
    project.save
    puts "Restored TunnelObfuscator.swift to WarrenRustRuntime"
  end
end
