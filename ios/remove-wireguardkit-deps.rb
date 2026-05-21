#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.4 : drop `WireGuardKit` + `WireGuardKitTypes` SPM package product
# dependencies from every target that declares them, and remove the
# matching `*.framework in Embed Frameworks` entries. The Warren
# `wireguard-apple/` submodule is a stub Package.swift (empty
# WireGuardKit + WireGuardKitTypes targets) ; once Warren tunnels via
# Quinn (warren-tunnel) and `WarrenQuinnTunnelImplementation`, the
# legacy WG framework deps are dead weight. Removing them resolves the
# `WireGuardKitTypes.modulemap not found` cascade that blocks
# `WarrenTypes` + dependents from compiling.
#
# This script does NOT delete the legacy WG source files
# (WgAdapter, WireGuardGoTunnelImplementation, etc.) ; those will fail
# to compile against the empty stub but are addressed in a separate
# C.4.4.X follow-up that either stubs them out target-side or removes
# them entirely once `WarrenQuinnTunnelImplementation` is the
# production path.
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/remove-wireguardkit-deps.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

WG_PRODUCT_NAMES = %w[WireGuardKit WireGuardKitTypes].freeze

removed_deps = []
removed_embeds = []

project.targets.each do |target|
  # PBXLegacyTarget (e.g. WireGuardGoBridge) doesn't carry these
  # attributes ; skip silently.
  next unless target.respond_to?(:package_product_dependencies)

  # Drop package product dependencies that reference WG packages.
  target.package_product_dependencies.dup.each do |dep|
    next unless WG_PRODUCT_NAMES.include?(dep.product_name)
    target.package_product_dependencies.delete(dep)
    removed_deps << "#{target.name} : #{dep.product_name}"
  end

  # Drop embed frameworks entries referencing WG packages.
  next unless target.respond_to?(:copy_files_build_phases)
  embed_phases = target.copy_files_build_phases.select { |p| p.symbol_dst_subfolder_spec == :frameworks }
  embed_phases.each do |phase|
    phase.files.dup.each do |build_file|
      ref = build_file.file_ref
      next unless ref
      display = ref.respond_to?(:display_name) ? ref.display_name : ref.to_s
      next unless WG_PRODUCT_NAMES.any? { |name| display.include?(name) }
      phase.files.delete(build_file)
      removed_embeds << "#{target.name} : #{display}"
    end
  end
end

# Drop the package reference itself from the project so SPM stops
# trying to build it.
project.root_object.package_references.dup.each do |pkg|
  next unless pkg.respond_to?(:repositoryURL) || pkg.respond_to?(:relative_path)
  source = pkg.respond_to?(:repositoryURL) ? pkg.repositoryURL : pkg.relative_path
  next unless source && source.to_s.include?("wireguard-apple")
  project.root_object.package_references.delete(pkg)
  removed_embeds << "project / package_reference : #{source}"
end

if removed_deps.empty? && removed_embeds.empty?
  puts "No WireGuardKit/Types deps or embeds to remove. No changes."
else
  project.save
  puts "Removed #{removed_deps.size} package product dependencies + #{removed_embeds.size} embed/package entries:"
  removed_deps.first(10).each { |d| puts "  - dep: #{d}" }
  removed_embeds.first(10).each { |e| puts "  - emb: #{e}" }
  if (removed_deps.size + removed_embeds.size) > 20
    puts "  ..."
  end
end
