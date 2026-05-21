#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.4.X : drop legacy WireGuard-using Swift source files from their
# target build phases. Files stay on disk (preserves git history) but
# stop being compiled. PacketTunnelProvider.swift was edited (C.4.4.X)
# to no longer reference `WireGuardGoTunnelImplementation` and now uses
# `WarrenQuinnTunnelImplementation` instead.
#
# Removed files (still on disk for git history):
# - PacketTunnel/WireGuardAdapter/WgAdapter.swift
# - PacketTunnel/WireGuardAdapter/WireGuardAdapter+Async.swift
# - PacketTunnel/WireGuardAdapter/WireGuardAdapterError+Localization.swift
# - PacketTunnel/WireGuardAdapter/WireGuardLogLevel+Logging.swift
# - PacketTunnel/PacketTunnelProvider/WireGuardGoTunnelImplementation.swift
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/remove-wireguardkit-sources.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

# File names (basename only) that must be dropped from every target's
# source build phase. Matching is by file_ref display name so we don't
# accidentally hit identically-named files in unrelated paths.
WG_SOURCE_BASENAMES = %w[
  WgAdapter.swift
  WireGuardAdapter+Async.swift
  WireGuardAdapterError+Localization.swift
  WireGuardLogLevel+Logging.swift
  WireGuardGoTunnelImplementation.swift
].freeze

removed = Hash.new { |h, k| h[k] = [] }

project.targets.each do |target|
  next unless target.respond_to?(:source_build_phase)
  phase = target.source_build_phase
  next unless phase

  phase.files.dup.each do |build_file|
    next unless build_file.file_ref
    name = build_file.file_ref.respond_to?(:display_name) ? build_file.file_ref.display_name : nil
    next unless name && WG_SOURCE_BASENAMES.include?(name)
    phase.files.delete(build_file)
    removed["sources:#{target.name}"] << name
  end
end

# Also drop any orphan PBXBuildFile entries that survived above (rare
# but happens when the file_ref was already detached by a prior run).
project.objects.dup.each do |obj|
  next unless obj.is_a?(Xcodeproj::Project::Object::PBXBuildFile)
  ref = obj.file_ref
  next unless ref && ref.respond_to?(:display_name)
  next unless WG_SOURCE_BASENAMES.include?(ref.display_name)
  obj.remove_from_project
  removed["orphan"] << ref.display_name
end

total = removed.values.flatten.size
if total.zero?
  puts "No WG legacy source files to remove. Project already clean."
else
  project.save
  puts "Removed #{total} entries from source build phases:"
  removed.each { |group, items| puts "  [#{group}] #{items.size} : #{items.uniq.join(", ")}" }
end
