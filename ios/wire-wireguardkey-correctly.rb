#!/usr/bin/env ruby
# frozen_string_literal: true

# Correct wire-up of WireGuardKey.swift after the over-aggressive
# remove + restore : ensure
#   - WarrenTypes/WireGuardKey.swift file_ref is in WarrenTypes target
#   - WarrenRustRuntime/WireGuardKey.swift file_ref is in WarrenRustRuntime target
# Drop any cross-wired entries.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# Find both file_refs by real_path.
types_ref = project.files.find { |f| f.real_path.to_s.end_with?("ios/WarrenTypes/WireGuardKey.swift") }
runtime_ref = project.files.find { |f| f.real_path.to_s.end_with?("ios/WarrenRustRuntime/WireGuardKey.swift") }

unless types_ref && runtime_ref
  warn "Missing file_refs : types=#{types_ref.inspect} runtime=#{runtime_ref.inspect}"
  exit 1
end

WANTED = {
  "WarrenTypes" => types_ref,
  "WarrenRustRuntime" => runtime_ref,
}.freeze

changed = []

WANTED.each do |target_name, wanted_ref|
  target = project.targets.find { |t| t.name == target_name }
  next unless target&.source_build_phase

  # Drop any build_file entries pointing to a WireGuardKey.swift
  # that is NOT the wanted one (cross-wiring cleanup).
  target.source_build_phase.files.dup.each do |bf|
    next unless bf.file_ref&.respond_to?(:display_name)
    next unless bf.file_ref.display_name == "WireGuardKey.swift"
    next if bf.file_ref == wanted_ref
    target.source_build_phase.files.delete(bf)
    changed << "drop wrong-path WireGuardKey from #{target_name}"
  end

  # Add the wanted file_ref if missing.
  unless target.source_build_phase.files_references.include?(wanted_ref)
    target.add_file_references([wanted_ref])
    changed << "add correct WireGuardKey to #{target_name}"
  end
end

if changed.empty?
  puts "WireGuardKey.swift wiring already correct. No changes."
else
  project.save
  puts "Changes :"
  changed.each { |c| puts "  + #{c}" }
end
