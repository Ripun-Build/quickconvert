// One action in the list (DESIGN §4): icon, label, shortcut. The highlight pill is shared (layoutId).
import { motion } from "framer-motion";
import { FileText, Image, Images, Layers, Minimize2, Scaling, SlidersHorizontal, type LucideIcon } from "lucide-react";
import type { ActionDef, ActionId } from "../lib/actions";
import { Kbd } from "./Kbd";

const ICONS: Record<ActionId, LucideIcon> = {
  images_to_pdf: FileText,
  pdf_to_images: Images,
  image_convert: Image,
  image_resize: Scaling,
  image_compress: Minimize2,
  pdf_merge: Layers,
  apply_preset: SlidersHorizontal,
};

interface Props {
  id: string;
  action: ActionDef;
  shortcut?: number;
  highlighted: boolean;
  onHover: () => void;
  onSelect: () => void;
}

export function ActionRow({ id, action, shortcut, highlighted, onHover, onSelect }: Props) {
  const Icon = ICONS[action.id];
  return (
    <div
      id={id}
      role="option"
      aria-selected={highlighted}
      onMouseMove={onHover}
      onClick={onSelect}
      className="relative flex h-9 cursor-default items-center gap-3 rounded-row px-[10px]"
    >
      {highlighted && (
        <motion.div
          layoutId="action-highlight"
          className="absolute inset-0 rounded-row bg-surface-selected"
          transition={{ type: "spring", stiffness: 520, damping: 40 }}
        />
      )}
      <Icon
        size={18}
        strokeWidth={1.75}
        className={`relative shrink-0 ${highlighted ? "text-accent" : "text-secondary"}`}
      />
      <span className="type-body relative flex-1 truncate text-primary">{action.label}</span>
      {shortcut !== undefined && (
        <span className="relative">
          <Kbd>{shortcut}</Kbd>
        </span>
      )}
    </div>
  );
}
