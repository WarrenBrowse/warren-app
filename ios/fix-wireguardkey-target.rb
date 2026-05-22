#!/usr/bin/env ruby
# frozen_string_literal: true

# Fix : the over-aggressive `restore-wireguardkey.rb` added the
# `WarrenRustRuntime/WireGuardKey.swift` file_ref to BOTH WarrenTypes
# AND WarrenRustRuntime targets, but WarrenTypes already has its own
# `WarrenTypes/WireGuardKey.swift` (the namespace + pure-Swift Codable
# wrappers). Two files named WireGuardKey.swift in the same target
# causes duplicate symbol definitions for `WireGuard.PrivateKey.init()`
# + `.publicKey`.
#
# Drop the WarrenRustRuntime path from the WarrenTypes target.
# Keep it in WarrenRustRuntime (where CryptoKit is available for the
# FFI extension).
#
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

PATH_TO_REMOVE = "WarrenRustRuntime/WireGuardKey.swift"
TARGET_TO_REMOVE_FROM = "WarrenTypes"

target = project.targets.find { |t| t.name == TARGET_TO_REMOVE_FROM }
unless target&.respond_to?(:source_build_phase) && target.source_build_phase
  warn "#{TARGET_TO_REMOVE_FROM} target / source_build_phase not found"
  exit 0
end

removed = 0
target.source_build_phase.files.dup.each do |build_file|
  next unless build_file.file_ref
  real = build_file.file_ref.real_path.to_s
  next unless real.end_with?(PATH_TO_REMOVE)
  target.source_build_phase.files.delete(build_file)
  removed += 1
end

if removed == 0
  puts "WarrenRustRuntime/WireGuardKey.swift already absent from WarrenTypes target. OK."
else
  project.save
  puts "Removed #{removed} build_file entr#{removed == 1 ? "y" : "ies"} : WarrenRustRuntime/WireGuardKey.swift from WarrenTypes target"
end
