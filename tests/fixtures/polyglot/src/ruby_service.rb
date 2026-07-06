require "json"

module Example
  class RubyService
    DEFAULT_NAME = "guest"

    def initialize(name = DEFAULT_NAME)
      @name = name
    end

    def render
      JSON.generate(name: @name)
    end
  end
end
