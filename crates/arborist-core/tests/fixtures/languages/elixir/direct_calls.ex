defmodule Fixture do
  def compute(value), do: value + 1
  def orchestrate(value), do: compute(value)
end
