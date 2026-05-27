import { CalendarDays } from "lucide-react";
import type { DayButtonProps } from "react-day-picker";

import { countRecords, countTodayRecords } from "@/components/clipboard/clipStudioHelpers";
import { Calendar } from "@/components/ui/calendar";
import { cn } from "@/lib/utils";
import type { ClipboardDateGroup } from "@/types/clipboard";

interface ClipStudioCalendarPanelProps {
  dates: ClipboardDateGroup[];
  frequentCount: number;
  selectedDate: string;
  today: string;
  onDateSelect: (date: string) => void;
}

const defaultSourceName = "系统剪贴板";

export function ClipStudioCalendarPanel(props: ClipStudioCalendarPanelProps) {
  const dateCounts = new Map(props.dates.map((group) => [group.date, group.count]));
  const focusDate = parseDateKey(props.selectedDate || props.today) ?? new Date();
  const recordDays = countRecordDaysInMonth(dateCounts, focusDate);
  const totalCount = countRecords(props.dates);

  return (
    <section className="clip-panel-view">
      <CalendarIntro />
      <SummaryCard dates={props.dates} frequentCount={props.frequentCount} today={props.today} />
      <CalendarGrid
        dateCounts={dateCounts}
        focusDate={focusDate}
        onDateSelect={props.onDateSelect}
        recordDays={recordDays}
        selectedDate={props.selectedDate}
      />
      <SourceCard totalCount={totalCount} />
    </section>
  );
}

function CalendarIntro() {
  return (
    <div className="panel-card">
      <h2>
        <CalendarDays className="size-4" />
        日期看板
      </h2>
      <p>保留原来的日期浏览方式。点击有标记日期后，中间列表会切换到当天记录。</p>
    </div>
  );
}

function SummaryCard({ dates, frequentCount, today }: Pick<ClipStudioCalendarPanelProps, "dates" | "frequentCount" | "today">) {
  return (
    <div className="panel-card">
      <div className="date-summary">
        <DateStat label="今日记录" value={countTodayRecords(dates, today)} />
        <DateStat label="高频复用" value={frequentCount} />
        <DateStat label="总记录" value={countRecords(dates)} />
      </div>
    </div>
  );
}

function CalendarGrid({
  dateCounts,
  focusDate,
  onDateSelect,
  recordDays,
  selectedDate,
}: {
  dateCounts: Map<string, number>;
  focusDate: Date;
  recordDays: number;
  selectedDate: string;
  onDateSelect: (date: string) => void;
}) {
  const selectedDateValue = parseDateKey(selectedDate) ?? undefined;

  return (
    <div className="panel-card calendar-board">
      <div className="calendar-board-head">
        <div>
          <span>Calendar</span>
          <b>{formatMonthLabel(focusDate)}</b>
        </div>
        <em>{recordDays} 天有记录</em>
      </div>
      <Calendar
        fixedWeeks
        hideNavigation
        mode="single"
        month={focusDate}
        selected={selectedDateValue}
        showOutsideDays
        weekStartsOn={1}
        className="clip-calendar"
        components={{
          DayButton: (dayButtonProps) => <RecordDayButton {...dayButtonProps} dateCounts={dateCounts} />,
        }}
        formatters={{
          formatCaption: formatMonthLabel,
          formatWeekdayName: formatWeekday,
        }}
        modifiers={{
          hasRecord: (date) => (dateCounts.get(formatDateKey(date)) ?? 0) > 0,
        }}
        modifiersClassNames={{
          hasRecord: "has-record",
        }}
        onDayClick={(date, modifiers) => {
          if (!modifiers.disabled && (dateCounts.get(formatDateKey(date)) ?? 0) > 0) {
            onDateSelect(formatDateKey(date));
          }
        }}
      />
    </div>
  );
}

function RecordDayButton(props: DayButtonProps & { dateCounts: Map<string, number> }) {
  const { dateCounts, day, modifiers, className, children, ...buttonProps } = props;
  const count = dateCounts.get(formatDateKey(day.date)) ?? 0;
  return (
    <button
      {...buttonProps}
      className={cn(className, "record-day-button")}
      disabled={count === 0}
      type="button"
    >
      <span>{children}</span>
      {count > 0 ? <small>{count}</small> : null}
      {modifiers.selected ? <span className="sr-only">已选中</span> : null}
    </button>
  );
}

function SourceCard({ totalCount }: { totalCount: number }) {
  return (
    <div className="panel-card">
      <h2>来源应用</h2>
      {totalCount === 0 ? <div className="mini-empty">暂无来源应用</div> : <SourceButton totalCount={totalCount} />}
    </div>
  );
}

function SourceButton({ totalCount }: { totalCount: number }) {
  return (
    <div className="source-list">
      <button className="source-button active" type="button">
        <span className="source-avatar">{defaultSourceName[0]}</span>
        <span className="source-copy">
          <b>{defaultSourceName}</b>
          <small>{totalCount} 条记录</small>
        </span>
        <span className="source-chevron">›</span>
      </button>
    </div>
  );
}

function DateStat({ label, value }: { label: string; value: number }) {
  return (
    <div className="date-stat">
      <b>{value}</b>
      <span>{label}</span>
    </div>
  );
}

function parseDateKey(dateKey: string) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateKey);
  if (!match) {
    return null;
  }
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}

function formatDateKey(date: Date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function formatMonthLabel(date: Date) {
  return `${date.getFullYear()} 年 ${String(date.getMonth() + 1).padStart(2, "0")} 月`;
}

function formatMonthPrefix(date: Date) {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function formatWeekday(date: Date) {
  return ["日", "一", "二", "三", "四", "五", "六"][date.getDay()];
}

function countRecordDaysInMonth(dateCounts: Map<string, number>, focusDate: Date) {
  const monthPrefix = formatMonthPrefix(focusDate);
  return [...dateCounts.keys()].filter((dateKey) => dateKey.startsWith(monthPrefix)).length;
}
