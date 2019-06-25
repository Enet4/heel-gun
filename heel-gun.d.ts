/// A heel-gun configuration file root.
interface Config {
    targets: TestTarget[]
}

/// A specific test target of the tool.
interface TestTarget {
    /// HTTP endpoint relative to the URI
    endpoint: string,
    /// HTTP method
    method: Method,
    /// The methods to randomly test
    args: TestArg[]
}

/// HTTP method
declare enum Method {
    GET,
    POST,
    PUT,
    DELETE
}

/// An argument for generating a piece of the request.
interface TestArg {
    type?: string
}

/// An argument that generates a part of the URI's path.
interface PathTestArg extends TestArg {
    type?: "path",
    generator: ArgGenerator

}

/// An argument that generates one of the URI's query string key-values.
interface QueryTestArg extends TestArg {
    type: "query"
    key: ArgGenerator,
    value: ArgGenerator
}

/// Argument generators describe the strategies for building (often random)
/// components of a request.
interface ArgGenerator {
    type?: string
}

/// Tries multiple random things, easy to use.
interface MagicArgGenerator extends ArgGenerator {
    type?: "magic"
}


/// Always provides the given string.
interface FixedArgGenerator extends ArgGenerator {
    type: "fixed",
    value: string
}

/// Chooses one of the given arguments at random.
interface ChoiceArgGenerator extends ArgGenerator {
    type: "choice",
    values: string[]
}

/// Chooses a random number from the given range.
interface IntRangeArgGenerator extends ArgGenerator {
    type: "range",
    /// low end, must be a 64-bit signed integer
    low: number,
    /// high end, must be a 64-bit signed integer
    high: number,
}

/// Builds a decimal numeric sequence with the given length.
interface NumericArgGenerator extends ArgGenerator {
    type: "numeric",
    /// the number of digits, must be an unsigned integer
    len: number
}

/// Builds an alphanumeric sequence with the given length.
interface AlphaNumericArgGenerator extends ArgGenerator {
    type: "alphanumeric",
    /// the number of digits, must be an unsigned integer
    len: number
}

/// Chooses one of the given generators at random (OR).
interface UnionArgGenerator extends ArgGenerator {
    type: "union",
    generators: ArgGenerator[]    
}
