// Types for the spec that checks the generated tables are current.
export interface GeneratedTable {
  file: string;
  expected: string;
}

export function generatedTables(repo: string): GeneratedTable[];
