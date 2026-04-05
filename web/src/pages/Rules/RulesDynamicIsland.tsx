import { useState, useRef, useEffect, useCallback } from "react";
import { theme, Badge } from "antd";
import { ThunderboltOutlined, TeamOutlined } from "@ant-design/icons";
import { getActiveSummary, type ActiveRuleItem } from "../../api/rules";
import { useRulesStore } from "../../stores/useRulesStore";

interface Props {
  onNavigateRule: (name: string, groupId: string | null) => void;
}

const DRAG_THRESHOLD = 4;

export default function RulesDynamicIsland({ onNavigateRule }: Props) {
  const { token } = theme.useToken();
  const [expanded, setExpanded] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [activeRules, setActiveRules] = useState<ActiveRuleItem[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef({
    startX: 0,
    startY: 0,
    startPosX: 0,
    startPosY: 0,
    hasMoved: false,
  });

  const rules = useRulesStore((s) => s.rules);

  useEffect(() => {
    let cancelled = false;
    getActiveSummary()
      .then((resp) => {
        if (!cancelled) setActiveRules(resp.rules);
      })
      .catch(() => {
        if (!cancelled) setActiveRules([]);
      });
    return () => {
      cancelled = true;
    };
  }, [rules]);

  useEffect(() => {
    if (!expanded) return;
    const handler = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setExpanded(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [expanded]);

  const initPosition = useCallback(() => {
    if (position !== null || !containerRef.current) return;
    const parent = containerRef.current.parentElement;
    if (!parent) return;
    const parentRect = parent.getBoundingClientRect();
    const rect = containerRef.current.getBoundingClientRect();
    setPosition({
      x: rect.left - parentRect.left,
      y: rect.top - parentRect.top,
    });
  }, [position]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (e.button !== 0) return;
      initPosition();

      const parent = containerRef.current?.parentElement;
      if (!parent || !containerRef.current) return;

      const parentRect = parent.getBoundingClientRect();
      const rect = containerRef.current.getBoundingClientRect();

      const currentX = rect.left - parentRect.left;
      const currentY = rect.top - parentRect.top;

      dragRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        startPosX: currentX,
        startPosY: currentY,
        hasMoved: false,
      };

      const handleMouseMove = (ev: MouseEvent) => {
        const dx = ev.clientX - dragRef.current.startX;
        const dy = ev.clientY - dragRef.current.startY;
        if (
          !dragRef.current.hasMoved &&
          Math.abs(dx) < DRAG_THRESHOLD &&
          Math.abs(dy) < DRAG_THRESHOLD
        ) {
          return;
        }
        dragRef.current.hasMoved = true;
        setDragging(true);

        const parentEl = containerRef.current?.parentElement;
        if (!parentEl || !containerRef.current) return;
        const pRect = parentEl.getBoundingClientRect();
        const cRect = containerRef.current.getBoundingClientRect();

        let newX = dragRef.current.startPosX + dx;
        let newY = dragRef.current.startPosY + dy;

        newX = Math.max(0, Math.min(newX, pRect.width - cRect.width));
        newY = Math.max(0, Math.min(newY, pRect.height - cRect.height));

        setPosition({ x: newX, y: newY });
      };

      const handleMouseUp = () => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        setDragging(false);

        if (!dragRef.current.hasMoved) {
          setExpanded((v) => !v);
        }
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [initPosition],
  );

  const positionStyle: React.CSSProperties =
    position !== null
      ? { left: position.x, top: position.y }
      : { top: 6, left: "50%", transform: "translateX(-50%)" };

  const ownRules = activeRules.filter((r) => !r.group_id);
  const groupRulesMap = new Map<string, { groupName: string; groupId: string; rules: ActiveRuleItem[] }>();
  for (const r of activeRules) {
    if (r.group_id) {
      if (!groupRulesMap.has(r.group_id)) {
        groupRulesMap.set(r.group_id, {
          groupName: r.group_name ?? r.group_id,
          groupId: r.group_id,
          rules: [],
        });
      }
      groupRulesMap.get(r.group_id)!.rules.push(r);
    }
  }

  return (
    <div
      ref={containerRef}
      style={{
        position: "absolute",
        ...positionStyle,
        display: "flex",
        justifyContent: "center",
        zIndex: 10,
        pointerEvents: "auto",
      }}
    >
      <div
        onMouseDown={handleMouseDown}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 8,
          padding: expanded ? "8px 20px" : "6px 16px",
          borderRadius: expanded ? 16 : 20,
          backgroundColor: token.colorBgElevated,
          border: `1px solid ${token.colorBorderSecondary}`,
          boxShadow: expanded
            ? token.boxShadowSecondary
            : "0 1px 3px rgba(0,0,0,0.08)",
          cursor: dragging ? "grabbing" : "grab",
          transition: dragging
            ? "none"
            : "all 0.3s cubic-bezier(0.4, 0, 0.2, 1)",
          userSelect: "none",
          minWidth: expanded ? 220 : "auto",
        }}
      >
        <Badge
          count={activeRules.length}
          size="small"
          color={
            activeRules.length > 0
              ? token.colorSuccess
              : token.colorTextDisabled
          }
          overflowCount={99}
        >
          <ThunderboltOutlined
            style={{
              fontSize: 14,
              color:
                activeRules.length > 0
                  ? token.colorSuccess
                  : token.colorTextDisabled,
            }}
          />
        </Badge>
        <span
          style={{
            fontSize: 13,
            fontWeight: 500,
            color: token.colorText,
          }}
        >
          {activeRules.length} active
        </span>
      </div>

      {expanded && activeRules.length > 0 && (
        <div
          style={{
            position: "absolute",
            top: "100%",
            left: "50%",
            transform: "translateX(-50%)",
            marginTop: 4,
            minWidth: 240,
            maxWidth: 380,
            maxHeight: 400,
            overflowY: "auto",
            backgroundColor: token.colorBgElevated,
            border: `1px solid ${token.colorBorderSecondary}`,
            borderRadius: 12,
            boxShadow: token.boxShadowSecondary,
            padding: "4px 0",
            animation: "islandFadeIn 0.2s ease",
          }}
        >
          {ownRules.length > 0 && (
            <>
              <div
                style={{
                  padding: "6px 16px 2px",
                  fontSize: 11,
                  fontWeight: 600,
                  color: token.colorTextDescription,
                  textTransform: "uppercase",
                  letterSpacing: 0.5,
                }}
              >
                My Rules
              </div>
              {ownRules.map((rule) => (
                <RuleRow
                  key={`own-${rule.name}`}
                  rule={rule}
                  token={token}
                  onClick={() => {
                    setExpanded(false);
                    onNavigateRule(rule.name, null);
                  }}
                />
              ))}
            </>
          )}
          {[...groupRulesMap.values()].map((group) => (
            <div key={group.groupId}>
              <div
                style={{
                  padding: "8px 16px 2px",
                  fontSize: 11,
                  fontWeight: 600,
                  color: token.colorTextDescription,
                  display: "flex",
                  alignItems: "center",
                  gap: 4,
                }}
              >
                <TeamOutlined style={{ fontSize: 11 }} />
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {group.groupName}
                </span>
              </div>
              {group.rules.map((rule) => (
                <RuleRow
                  key={`${group.groupId}-${rule.name}`}
                  rule={rule}
                  token={token}
                  onClick={() => {
                    setExpanded(false);
                    onNavigateRule(rule.name, rule.group_id);
                  }}
                />
              ))}
            </div>
          ))}
        </div>
      )}

      {expanded && activeRules.length === 0 && (
        <div
          style={{
            position: "absolute",
            top: "100%",
            left: "50%",
            transform: "translateX(-50%)",
            marginTop: 4,
            minWidth: 200,
            backgroundColor: token.colorBgElevated,
            border: `1px solid ${token.colorBorderSecondary}`,
            borderRadius: 12,
            boxShadow: token.boxShadowSecondary,
            padding: "16px 20px",
            textAlign: "center",
            fontSize: 13,
            color: token.colorTextDescription,
          }}
        >
          No active rules
        </div>
      )}
    </div>
  );
}

function RuleRow({
  rule,
  token,
  onClick,
}: {
  rule: ActiveRuleItem;
  token: ReturnType<typeof theme.useToken>["token"];
  onClick: () => void;
}) {
  return (
    <div
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      style={{
        padding: "8px 16px",
        fontSize: 13,
        color: token.colorText,
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        gap: 8,
        transition: "background-color 0.15s",
      }}
      onMouseEnter={(e) => {
        (e.currentTarget as HTMLDivElement).style.backgroundColor =
          token.colorBgTextHover;
      }}
      onMouseLeave={(e) => {
        (e.currentTarget as HTMLDivElement).style.backgroundColor =
          "transparent";
      }}
    >
      <ThunderboltOutlined
        style={{ fontSize: 12, color: token.colorSuccess, flexShrink: 0 }}
      />
      <span
        style={{
          flex: 1,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {rule.name}
      </span>
      <span
        style={{
          fontSize: 12,
          color: token.colorTextDescription,
          flexShrink: 0,
        }}
      >
        {rule.rule_count} rules
      </span>
    </div>
  );
}
