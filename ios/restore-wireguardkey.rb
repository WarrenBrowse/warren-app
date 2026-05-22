#!/usr/bin/env ruby
# frozen_string_literal: true

# Restore WireGuardKey.swift to its 2 build targets (WarrenTypes +
# WarrenRustRuntime) after over-aggressive removal in
# `remove-mullvad-legacy-sources.rb`. The file provides the
# `WireGuard.PublicKey` namespace used by `WarrenTypes.RESTTypes.Device`
# — removing it cascades into 6+ compile errors.
#
# Idempotent. The file_ref already exists in the project (we only
# dropped the PBXBuildFile entries, not the FileRef).

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

TARGETS = %w[WarrenTypes WarrenRustRuntime].freeze
FILE_BASENAME = "WireGuardKey.swift"

# Find the existing file_ref.
file_ref = project.files.find do |f|
  f.respond_to?(:display_name) && f.display_name == FILE_BASENAME
end

unless file_ref
  warn "WireGuardKey.swift file_ref not found ; nothing to restore"
  exit 0
end

restored = []
TARGETS.each do |target_name|
  target = project.targets.find { |t| t.name == target_name }
  next unless target&.respond_to?(:source_build_phase) && target.source_build_phase
  next if target.source_build_phase.files_references.include?(file_ref)
  target.add_file_references([file_ref])
  restored << target_name
end

if restored.empty?
  puts "WireGuardKey.swift already in all required targets. No changes."
else
  project.save
  puts "Restored WireGuardKey.swift to #{restored.size} targets : #{restored.join(", ")}"
end
