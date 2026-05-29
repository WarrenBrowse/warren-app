#!/usr/bin/env ruby
# frozen_string_literal: true

# Warren: remove the dead legacy WireGuard tunnel backend from the Xcode
# project. Targets ONLY the WireGuardGo backend, the WireGuardAdapter
# group and the `wireguard-apple` SPM stub. Deliberately PRESERVES
# WireGuardKey.swift / WireGuardKeyTests.swift (shared x25519 key types)
# and WireGuardObfuscationSettings.swift (obfuscation, reused by Warren).

require 'xcodeproj'

project_path = File.expand_path('WarrenVPN.xcodeproj', __dir__)
project = Xcodeproj::Project.open(project_path)

DEAD_FILES = %w[
  WgAdapter.swift
  WireGuardAdapter+Async.swift
  WireGuardAdapterError+Localization.swift
  WireGuardLogLevel+Logging.swift
  WireGuardGoTunnelImplementation.swift
].freeze

DEAD_GROUPS = %w[WireGuardAdapter].freeze
DEAD_TARGETS = %w[WireGuardGoBridge].freeze
DEAD_PACKAGES = %w[wireguard-apple].freeze
DEAD_PRODUCTS = %w[WireGuardKit WireGuardKitTypes].freeze

removed = []

# 1. Remove build files referencing dead source files from all build phases.
project.targets.each do |t|
  t.build_phases.each do |ph|
    next unless ph.respond_to?(:files)

    ph.files.dup.each do |bf|
      fr = bf.file_ref
      name = fr.respond_to?(:display_name) ? fr.display_name : nil
      if name && DEAD_FILES.include?(File.basename(name.to_s))
        removed << "buildfile #{name} (#{t.name}/#{ph.display_name})"
        ph.remove_build_file(bf)
      end
    end
  end
end

# 2. Remove dead package product dependencies (WireGuardKit*) from targets,
#    plus any Frameworks build files that reference them.
project.targets.each do |t|
  if t.respond_to?(:package_product_dependencies)
    t.package_product_dependencies.dup.each do |dep|
      if DEAD_PRODUCTS.include?(dep.product_name.to_s)
        removed << "package-product #{dep.product_name} (#{t.name})"
        dep.remove_from_project
      end
    end
  end
  t.build_phases.each do |ph|
    next unless ph.respond_to?(:files)

    ph.files.dup.each do |bf|
      pd = bf.respond_to?(:product_ref) ? bf.product_ref : nil
      if pd && DEAD_PRODUCTS.include?(pd.product_name.to_s)
        removed << "framework-buildfile #{pd.product_name} (#{t.name})"
        ph.remove_build_file(bf)
      end
    end
  end
end

# 3. Remove dead file references + the WireGuardAdapter group.
project.files.dup.each do |fr|
  base = File.basename(fr.path.to_s)
  if DEAD_FILES.include?(base)
    removed << "fileref #{fr.path}"
    fr.remove_from_project
  end
end

project.main_group.recursive_children.dup.each do |child|
  next unless child.is_a?(Xcodeproj::Project::Object::PBXGroup)

  if DEAD_GROUPS.include?(child.display_name.to_s)
    removed << "group #{child.display_name}"
    child.remove_from_project
  end
end

# 4. Remove the WireGuardGoBridge native target, its dependencies + proxies.
project.targets.each do |t|
  next unless t.respond_to?(:dependencies)

  t.dependencies.dup.each do |dep|
    dep_name = dep.target ? dep.target.name : dep.display_name
    if DEAD_TARGETS.include?(dep_name.to_s)
      removed << "target-dependency -> #{dep_name} (#{t.name})"
      dep.remove_from_project
    end
  end
end

project.targets.select { |t| DEAD_TARGETS.include?(t.name.to_s) }.each do |t|
  # Drop any build files that embed/copy this target's product.
  product = t.respond_to?(:product_reference) ? t.product_reference : nil
  if product
    project.targets.each do |other|
      other.build_phases.each do |ph|
        next unless ph.respond_to?(:files)

        ph.files.dup.each do |bf|
          if bf.file_ref == product
            removed << "product-buildfile #{t.name} (#{other.name}/#{ph.display_name})"
            ph.remove_build_file(bf)
          end
        end
      end
    end
  end
  removed << "target #{t.name}"
  t.remove_from_project
end

# 5. Remove the wireguard-apple local SPM package reference.
root = project.root_object
if root.respond_to?(:package_references)
  root.package_references.dup.each do |pkg|
    path_attr = pkg.respond_to?(:relative_path) ? pkg.relative_path.to_s : ''
    repo_attr = pkg.respond_to?(:repositoryURL) ? pkg.repositoryURL.to_s : ''
    if DEAD_PACKAGES.any? { |n| path_attr.include?(n) || repo_attr.include?(n) }
      removed << "package-ref #{path_attr}#{repo_attr}"
      pkg.remove_from_project
    end
  end
end

project.save

puts "Removed #{removed.length} pbxproj entries:"
removed.each { |r| puts "  - #{r}" }
