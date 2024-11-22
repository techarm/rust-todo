import { Label } from '@/types/todo';

export const toggleLabels = (labels: Label[], target: Label) => {
  return labels.find(({ id }) => id === target.id) ? labels.filter(({ id }) => id !== target.id) : [...labels, target];
};
