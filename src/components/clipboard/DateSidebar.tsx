import type { ClipboardDateGroup } from "@/types/clipboard";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ClipboardList, Pause, Play } from "lucide-react";

interface DateSidebarProps {
  dates: ClipboardDateGroup[];
  selectedDate: string;
  today: string;
  monitorEnabled: boolean;
  onDateSelect: (date: string) => void;
  onMonitorToggle: () => void;
}

export function DateSidebar(props: DateSidebarProps) {
  return (
    <Card className="min-h-0 gap-4 overflow-hidden border-border/70 bg-card/90 shadow-xl backdrop-blur">
      <SidebarHeader />
      <CardContent className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
        <MonitorButton enabled={props.monitorEnabled} onToggle={props.onMonitorToggle} />
        <TodayButton {...props} />
        <DateList {...props} />
      </CardContent>
    </Card>
  );
}

function SidebarHeader() {
  return (
    <CardHeader>
      <CardDescription>Local Clipboard</CardDescription>
      <CardTitle className="flex items-center gap-2 text-2xl">
        <ClipboardList className="size-6 text-primary" />
        剪贴板工具箱
      </CardTitle>
    </CardHeader>
  );
}

function MonitorButton({ enabled, onToggle }: { enabled: boolean; onToggle: () => void }) {
  return (
    <Button
      className="h-auto justify-between rounded-xl px-3 py-3 text-left"
      variant={enabled ? "secondary" : "destructive"}
      onClick={onToggle}
    >
      <span>{enabled ? "监听中" : "已暂停"}</span>
      {enabled ? <Pause className="size-4" /> : <Play className="size-4" />}
    </Button>
  );
}

function TodayButton({ selectedDate, today, onDateSelect }: DateSidebarProps) {
  return (
    <Button
      className="h-auto justify-start rounded-xl px-3 py-3 text-left"
      variant={selectedDate === today ? "default" : "secondary"}
      onClick={() => onDateSelect(today)}
    >
      <span className="flex flex-col items-start gap-1">
        <span className="text-sm font-semibold">今天</span>
        <span className="text-xs opacity-80">{today}</span>
      </span>
    </Button>
  );
}

function DateList({ dates, selectedDate, onDateSelect }: DateSidebarProps) {
  return (
    <div className="min-h-0 space-y-2 overflow-auto pr-1">
      {dates.map((group) => (
        <Button
          className="h-auto w-full justify-between rounded-xl px-3 py-3"
          key={group.date}
          variant={selectedDate === group.date ? "default" : "ghost"}
          onClick={() => onDateSelect(group.date)}
        >
          <span>{group.date}</span>
          <Badge variant={selectedDate === group.date ? "secondary" : "outline"}>
            {group.count} 条
          </Badge>
        </Button>
      ))}
    </div>
  );
}
