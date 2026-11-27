import { describe, expect, it } from 'vitest';

import {
  FORUM_REPORT_RECORDS_MAX,
  forumReportSummary,
  IForumReportRecord,
  recordForumReport,
  sanitizeForumReportRecords,
} from '../../src/shared/forum-report';

const record = (topicId: number, createdAt = '2026-11-27T10:00:00.000Z'): IForumReportRecord => ({
  topicId,
  summary: `report ${topicId}`,
  createdAt,
});

describe('the list of reports this app filed', () => {
  it('puts the newest first and keeps one entry per topic', () => {
    const list = recordForumReport([record(1), record(2)], record(1, '2026-11-28T10:00:00.000Z'));
    expect(list.map((r) => r.topicId)).toEqual([1, 2]);
    expect(list[0].createdAt).toBe('2026-11-28T10:00:00.000Z');
  });

  it('forgets the oldest past the cap', () => {
    let list: IForumReportRecord[] = [];
    for (let id = 1; id <= FORUM_REPORT_RECORDS_MAX + 3; id++) {
      list = recordForumReport(list, record(id));
    }
    expect(list).toHaveLength(FORUM_REPORT_RECORDS_MAX);
    expect(list[0].topicId).toBe(FORUM_REPORT_RECORDS_MAX + 3);
    expect(list.map((r) => r.topicId)).not.toContain(1);
  });
});

describe('reading the stored list back', () => {
  it('keeps only well-formed entries, so a damaged file cannot break the screen', () => {
    expect(
      sanitizeForumReportRecords([
        record(7),
        { topicId: -1, summary: 'x', createdAt: '2026-11-27T10:00:00.000Z' },
        { topicId: 0, summary: 'x', createdAt: '2026-11-27T10:00:00.000Z' },
        { topicId: 2 ** 53, summary: 'x', createdAt: '2026-11-27T10:00:00.000Z' },
        { topicId: 8, summary: 3, createdAt: '2026-11-27T10:00:00.000Z' },
        { topicId: 9, summary: 'x', createdAt: 'yesterday' },
        'garbage',
        null,
      ]),
    ).toEqual([record(7)]);
  });

  it('answers an empty list for anything that is not a list', () => {
    expect(sanitizeForumReportRecords(undefined)).toEqual([]);
    expect(sanitizeForumReportRecords({ topicId: 1 })).toEqual([]);
  });

  it('never holds more than the cap, whatever the file says', () => {
    const many = Array.from({ length: FORUM_REPORT_RECORDS_MAX + 5 }, (_, i) => record(i + 1));
    expect(sanitizeForumReportRecords(many)).toHaveLength(FORUM_REPORT_RECORDS_MAX);
  });
});

describe('the line a report is listed under', () => {
  it('is the first words of the description on one line, clipped', () => {
    expect(forumReportSummary('  The VPN\n\ndrops   every evening  ')).toBe(
      'The VPN drops every evening',
    );
    const long = 'a'.repeat(200);
    const summary = forumReportSummary(long);
    expect(Array.from(summary)).toHaveLength(61);
    expect(summary.endsWith('…')).toBe(true);
  });
});
