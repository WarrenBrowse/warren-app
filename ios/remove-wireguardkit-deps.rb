#!/usr/bin/env ruby
# frozen_string_literal: true

# C.4.4 : drop ALL `WireGuardKit` + `WireGuardKitTypes` references from
# the Xcode project. Once Warren tunnels via Quinn (warren-tunnel) and
# `WarrenQuinnTunnelImplementation` (C.4.3), the legacy WG framework
# deps are dead weight. Removing them resolves the
# `WireGuardKitTypes.modulemap not found` cascade that blocks
# `WarrenTypes` + dependents from compiling.
#
# Removes :
# - `XCSwiftPackageProductDependency` PBXBuildFile entries
# - `Frameworks` (Link Binary With Libraries) phase entries
# - `Embed Frameworks` (Copy Files) phase entries
# - `packageProductDependencies` arrays on each target
# - `package_references` on the project root (wireguard-apple)
#
# Does NOT touch the legacy WG source files (WgAdapter,
# WireGuardGoTunnelImplementation, etc.). Those will fail to compile
# against the empty stub once the framework deps are gone ; they need
# to be either stubbed out target-side or removed entirely. See
# `remove-wireguardkit-sources.rb` (C.4.4.X follow-up).
#
# Usage : /opt/homebrew/opt/ruby/bin/ruby ios/remove-wireguardkit-deps.rb
# Idempotent.

require "xcodeproj"

SCRIPT_DIR = File.dirname(File.expand_path(__FILE__))
PROJECT_PATH = File.join(SCRIPT_DIR, "WarrenVPN.xcodeproj")
project = Xcodeproj::Project.open(PROJECT_PATH)

WG_PRODUCT_NAMES = %w[WireGuardKit WireGuardKitTypes].freeze

# True if a PBXBuildFile or PBXSwiftPackageProductDependency points at
# one of the legacy WG products. Robust against missing methods.
def matches_wg?(obj, names)
  candidates = []
  candidates << obj.product_name if obj.respond_to?(:product_name)
  candidates << obj.display_name if obj.respond_to?(:display_name)
  if obj.respond_to?(:product_ref) && obj.product_ref
    candidates << obj.product_ref.product_name if obj.product_ref.respond_to?(:product_name)
  end
  if obj.respond_to?(:file_ref) && obj.file_ref
    candidates << obj.file_ref.display_name if obj.file_ref.respond_to?(:display_name)
  end
  candidates.compact!
  candidates.any? { |c| names.any? { |n| c.to_s == n || c.to_s.start_with?("#{n} ") } }
end

removed = Hash.new { |h, k| h[k] = [] }

project.targets.each do |target|
  next unless target.respond_to?(:package_product_dependencies)

  # 1. packageProductDependencies array.
  target.package_product_dependencies.dup.each do |dep|
    next unless WG_PRODUCT_NAMES.include?(dep.product_name)
    target.package_product_dependencies.delete(dep)
    removed["dep:#{target.name}"] << dep.product_name
  end

  # 2. Frameworks build phase (link).
  if target.respond_to?(:frameworks_build_phase) && target.frameworks_build_phase
    target.frameworks_build_phase.files.dup.each do |build_file|
      next unless matches_wg?(build_file, WG_PRODUCT_NAMES)
      target.frameworks_build_phase.files.delete(build_file)
      removed["link:#{target.name}"] << build_file.display_name
    end
  end

  # 3. Copy Files (Embed Frameworks) phases.
  if target.respond_to?(:copy_files_build_phases)
    target.copy_files_build_phases.each do |phase|
      phase.files.dup.each do |build_file|
        next unless matches_wg?(build_file, WG_PRODUCT_NAMES)
        phase.files.delete(build_file)
        removed["embed:#{target.name}"] << build_file.display_name
      end
    end
  end
end

# 4. Drop orphan top-level PBXBuildFile + PBXSwiftPackageProductDependency
#    objects referencing WG products. Iterate the project's objects.
project.objects.dup.each do |obj|
  next unless obj.is_a?(Xcodeproj::Project::Object::PBXBuildFile) ||
              obj.is_a?(Xcodeproj::Project::Object::XCSwiftPackageProductDependency)
  next unless matches_wg?(obj, WG_PRODUCT_NAMES)
  obj.remove_from_project
  removed["orphan"] << obj.uuid
end

# 5. Drop the wireguard-apple package reference from the project root.
project.root_object.package_references.dup.each do |pkg|
  source = pkg.respond_to?(:repositoryURL) ? pkg.repositoryURL : nil
  source ||= pkg.respond_to?(:relative_path) ? pkg.relative_path : nil
  next unless source && source.to_s.include?("wireguard-apple")
  project.root_object.package_references.delete(pkg)
  removed["package_ref"] << source.to_s
end

total = removed.values.flatten.size
if total.zero?
  puts "No WireGuardKit/Types entries to remove. Project already clean."
else
  project.save
  puts "Removed #{total} entries:"
  removed.each do |group, items|
    puts "  [#{group}] #{items.size}"
    items.first(3).each { |i| puts "    - #{i}" }
    puts "    ..." if items.size > 3
  end
end
